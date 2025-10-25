use crate::db::error::DbError;
use crate::error::{AppResult, DomainError, InfraError};
use crate::hardening::{ALLOWED_DIRS, FORBIDDEN_LUA_TOKENS, MAX_LUA_BYTES};
use crate::models::types::BlueprintId;
use crate::util::{list_yaml_files_guarded, resolve_content_subdir};
use mlua::Lua;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::{fs, path::Path};
use tokio_postgres::Transaction;
use crate::lua::ScriptHook;
// ====== v2 YAML models ======

#[derive(Debug, Deserialize)]
struct RoomYaml {
    pub version: u8,  // must be 2
    pub id: String,   // "entry"
    pub name: String, // "Entry Hall"
    #[serde(default)]
    pub short: Option<String>,
    #[serde(rename = "description")]
    pub full_desc: String,
    #[serde(default)]
    pub state: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub hints: Vec<HintYaml>,
    #[serde(default)]
    pub objects: Vec<ObjectYaml>,
    #[serde(default)]
    pub exits: Vec<ExitYaml>,
    #[serde(default)]
    pub scripts: ScriptYaml,
    // optional items catalog ignored here (handled elsewhere if you add a table)
}

#[derive(Debug, Deserialize, Serialize)]
struct HintYaml {
    pub text: String,
    #[serde(default)]
    pub when: Option<String>, // "enter" | "first_look" | "manual" | ...
    #[serde(default)]
    pub cooldown: Option<i32>,
    #[serde(default)]
    pub once: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ObjectYaml {
    pub id: String, // object key (used as name)
    #[serde(default)]
    pub nouns: Vec<String>,
    pub short: String,
    pub description: String,
    #[serde(default)]
    pub examine: Option<String>,

    #[serde(default)]
    pub flags: Vec<String>, // ["overlay","non_stackable"]
    #[serde(default)]
    pub visible: Option<VisiblePolicy>, // always|when_revealed|when_unlocked|script

    #[serde(default)]
    pub state: HashMap<String, serde_json::Value>, // arbitrary map (locked, revealed, etc)
    #[serde(default)]
    pub controls: Vec<String>, // ["exit:north.locked","object:door.locked"]

    #[serde(default)]
    pub loot: Option<serde_json::Value>, // {"items":[...],"credits":0,"once":true}

    #[serde(default)]
    pub use_: Option<String>, // Lua (key "use" in YAML)
    #[serde(rename = "use", default)]
    pub _use_compat: Option<String>, // compat alias
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum VisiblePolicy {
    Always,
    WhenRevealed,
    WhenUnlocked,
    Script,
}

#[derive(Debug, Deserialize)]
struct ExitYaml {
    pub dir: String, // "north"
    pub to: String,  // "hallway_1"
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub locked: Option<bool>,
    #[serde(default)]
    pub visible_when_locked: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScriptYaml(HashMap<ScriptHook, String>);

impl Default for ScriptYaml {
    fn default() -> Self {
        ScriptYaml(HashMap::new())
    }
}

// ====== Entry point ======

pub async fn import_blueprint_sub_dir(
    blueprint_id: BlueprintId,
    sub_dir: &str,
    content_base: &Path,
    db: &crate::db::Db,
) -> AppResult<()> {
    let dir = resolve_content_subdir(content_base, sub_dir)?;
    let files = list_yaml_files_guarded(&dir)?;

    // Parse first (so we can do multi-pass write)
    let mut rooms: Vec<RoomYaml> = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).map_err(InfraError::from)?;
        let mut room: RoomYaml = serde_yaml::from_str(&text)?;
        // normalize "use"
        for o in &mut room.objects {
            if o.use_.is_none() {
                o.use_ = o._use_compat.take();
            }
        }
        validate_room_semantics(&room)?;
        validate_lua_for_room(&room)?; // compile-check
        rooms.push(room);
    }

    let mut client = db.pool.get().await.map_err(DbError::from)?;
    let tx = client.build_transaction().start().await.map_err(DbError::from)?;

    // Pass 1: upsert rooms, collect ids
    let mut room_ids: HashMap<String, uuid::Uuid> = HashMap::new();
    for r in &rooms {
        let room_id = upsert_room_header(&tx, blueprint_id, r).await?;
        room_ids.insert(r.id.clone(), room_id);
    }

    // Pass 2: kv, objects (+nouns, scripts), room scripts
    for r in &rooms {
        let room_id = *room_ids.get(&r.id).expect("room id present");
        upsert_room_kv(&tx, room_id, &r.state).await?;
        upsert_objects(&tx, room_id, &r.objects).await?;
        upsert_room_scripts(&tx, room_id, &r.scripts).await?;
    }

    // Pass 3: exits (needs both from/to room ids)
    for r in &rooms {
        let from_room_id = *room_ids.get(&r.id).unwrap();
        upsert_exits(&tx, from_room_id, &r.exits, &room_ids).await?;
    }

    tx.commit().await.map_err(DbError::from)?;
    Ok(())
}

// ====== DB writers ======

async fn upsert_room_header(tx: &Transaction<'_>, bp_id: BlueprintId, r: &RoomYaml) -> AppResult<uuid::Uuid> {
    let title = &r.name;
    let short = r.short.as_deref().unwrap_or_default();
    let body = &r.full_desc;

    // Store hints as JSON (structured v2)
    let hints_json = serde_json::to_value(&r.hints)?;

    // Insert/update by (bp_id, key), return id
    let row = tx
        .query_one(
            r#"
            INSERT INTO bp_rooms (bp_id, key, title, short, body, hints)
            VALUES ($1,$2,$3,$4,$5,$6::jsonb)
            ON CONFLICT (bp_id, key) DO UPDATE
            SET title = EXCLUDED.title,
                short = EXCLUDED.short,
                body  = EXCLUDED.body,
                hints = EXCLUDED.hints
            RETURNING id
            "#,
            &[&bp_id, &r.id, &title, &short, &body, &hints_json],
        )
        .await
        .map_err(DbError::from)?;
    Ok(row.get(0))
}

async fn upsert_room_kv(
    tx: &Transaction<'_>,
    room_id: uuid::Uuid,
    kv: &HashMap<String, serde_json::Value>,
) -> AppResult<()> {
    // Simple strategy: replace all kv for the room (small dataset)
    tx.execute("DELETE FROM bp_room_kv WHERE room_id = $1", &[&room_id])
        .await
        .map_err(DbError::from)?;
    for (k, v) in kv {
        tx.execute(
            r#"
            INSERT INTO bp_room_kv (room_id, key, value)
            VALUES ($1,$2,$3)
            ON CONFLICT (room_id, key) DO UPDATE SET value = EXCLUDED.value
            "#,
            &[&room_id, k, v],
        )
        .await
        .map_err(DbError::from)?;
    }
    Ok(())
}

async fn upsert_objects(tx: &Transaction<'_>, room_id: uuid::Uuid, objects: &[ObjectYaml]) -> AppResult<()> {
    // Replace all (keeps code simple & deterministic ordering via position)
    tx.execute("DELETE FROM bp_object_nouns WHERE room_id = $1", &[&room_id])
        .await
        .map_err(DbError::from)?;
    tx.execute("DELETE FROM bp_objects WHERE room_id = $1", &[&room_id])
        .await
        .map_err(DbError::from)?;

    for (pos, o) in objects.iter().enumerate() {
        let state_json = &o.state;
        let flags_json = serde_json::to_value(&o.flags)?;
        let controls_json = serde_json::to_value(&o.controls)?;
        let loot_json = serde_json::to_value(&o.loot)?;

        // visible as text (enum) or NULL
        let visible_txt: Option<&'static str> = match o.visible {
            Some(VisiblePolicy::Always) => Some("always"),
            Some(VisiblePolicy::WhenRevealed) => Some("when_revealed"),
            Some(VisiblePolicy::WhenUnlocked) => Some("when_unlocked"),
            Some(VisiblePolicy::Script) => Some("script"),
            None => None,
        };

        let row = tx
            .query_one(
                r#"
                INSERT INTO bp_objects
                    (room_id, name, short, description, examine, use_lua,
                    position, flags, visible, controls, loot)
                VALUES
                    ($1,$2,$3,$4,$5,$6,$7,$8::jsonb,$9,$10::jsonb,$11::jsonb)
                RETURNING id
                "#,
                &[
                    &room_id,
                    &o.id,
                    &o.short,
                    &o.description,
                    &o.examine,
                    &o.use_,
                    &(pos as i32),
                    &flags_json,
                    &visible_txt,
                    &controls_json,
                    &loot_json,
                ],
            )
            .await
            .map_err(DbError::from)?;
        let obj_id: uuid::Uuid = row.get(0);

        // state
        for (k, v) in state_json {
            tx.execute(
                r#"
                INSERT INTO bp_objects_kv (object_id, key, value)
                VALUES ($1,$2,$3)
                ON CONFLICT (object_id, key) DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&obj_id, k, v],
            )
            .await
            .map_err(DbError::from)?;
        }

        // nouns
        for n in &o.nouns {
            tx.execute(
                r#"
                INSERT INTO bp_object_nouns (room_id, obj_id, noun)
                VALUES ($1,$2,$3)
                ON CONFLICT (room_id, noun) DO UPDATE SET obj_id = EXCLUDED.obj_id
                "#,
                &[&room_id, &obj_id, n],
            )
            .await
            .map_err(DbError::from)?;
        }
    }

    Ok(())
}

async fn upsert_room_scripts(tx: &Transaction<'_>, room_id: uuid::Uuid, scripts: &ScriptYaml) -> AppResult<()> {
    // single-row table keyed by room_id
    for (_, (hook, script)) in scripts.0.iter().enumerate() {
        dbg!(&hook);
        tx.execute(
            r#"
            INSERT INTO bp_room_scripts (room_id, hook, script)
            VALUES ($1,$2,$3)
            ON CONFLICT (room_id, hook) DO UPDATE SET script = EXCLUDED.script
            "#,
        &[&room_id, &hook.as_str(), &script],
        )
            .await
            .map_err(DbError::from)?;
    }
    Ok(())
}

async fn upsert_exits(
    tx: &Transaction<'_>,
    from_room_id: uuid::Uuid,
    exits: &Vec<ExitYaml>,
    key_to_id: &HashMap<String, uuid::Uuid>,
) -> AppResult<()> {
    for ex in exits {
        let d = ex.dir.to_ascii_lowercase();
        let to_room_id = *key_to_id.get(&ex.to).ok_or_else(|| DomainError::Validation {
            field: "exit",
            message: format!("unknown target room key '{}'", ex.to),
        })?;

        tx.execute(
            r#"
            INSERT INTO bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
            VALUES ($1,$2,$3, COALESCE($4,false), $5, COALESCE($6,true))
            ON CONFLICT (from_room_id, dir) DO UPDATE
            SET to_room_id = EXCLUDED.to_room_id,
                locked = EXCLUDED.locked,
                description = EXCLUDED.description,
                visible_when_locked = EXCLUDED.visible_when_locked
            "#,
            &[
                &from_room_id,
                &d,
                &to_room_id,
                &ex.locked,
                &ex.description,
                &ex.visible_when_locked,
            ],
        )
        .await
        .map_err(DbError::from)?;
    }
    Ok(())
}

// ====== Validation & Lua compile ======

fn validate_room_semantics(room: &RoomYaml) -> AppResult<()> {
    if room.version != 2 {
        return Err(DomainError::Validation {
            field: "room.version",
            message: "unsupported room schema version; expected 2".into(),
        });
    }
    if room.id.trim().is_empty() {
        return Err(DomainError::Validation {
            field: "room",
            message: "room id empty".into(),
        });
    }
    if room.name.trim().is_empty() {
        return Err(DomainError::Validation {
            field: "room",
            message: "room name empty".into(),
        });
    }
    if room.full_desc.trim().is_empty() {
        return Err(DomainError::Validation {
            field: "room",
            message: "room desc empty".into(),
        });
    }
    if room.id.len() > 64 {
        return Err(DomainError::Validation {
            field: "room",
            message: "room id too long".into(),
        });
    }
    if room.name.len() > 128 {
        return Err(DomainError::Validation {
            field: "room",
            message: "room name too long".into(),
        });
    }

    // object ids unique
    let mut ids = HashSet::new();
    for o in &room.objects {
        if o.id.trim().is_empty() {
            return Err(DomainError::Validation {
                field: "object",
                message: "object with empty id".into(),
            });
        }
        if !ids.insert(&o.id) {
            return Err(DomainError::Validation {
                field: "object",
                message: format!("duplicate object id: {}", o.id),
            });
        }
        // visible enum validated by serde; nothing to do here
        // controls format: "exit:<dir>.<field>" or "object:<id>.<path>"
        for c in &o.controls {
            let ok = c.starts_with("exit:") || c.starts_with("object:");
            if !ok {
                return Err(DomainError::Validation {
                    field: "object.controls",
                    message: format!("invalid control address '{}'", c),
                });
            }
        }
    }

    // {o:ID} placeholders must reference existing objects (check both description + optional 'o' field)
    let re = Regex::new(r"\{o:([a-zA-Z0-9_\-]+)}").unwrap();
    for src in [room.full_desc.as_str()].into_iter() {
        for cap in re.captures_iter(src) {
            let id = cap[1].to_string();
            if !ids.contains(&id) {
                return Err(DomainError::Validation {
                    field: "description",
                    message: format!("text references unknown object id: {}", id),
                });
            }
        }
    }

    // exits: dir whitelist, 'to' slug-ish
    let slug = Regex::new(r"^[a-zA-Z0-9_\-:]+$").unwrap();
    for ex in &room.exits {
        let d = ex.dir.to_ascii_lowercase();
        if !ALLOWED_DIRS.contains(&d.as_str()) {
            return Err(DomainError::Validation {
                field: "exit",
                message: format!("invalid exit dir '{}'", d),
            });
        }
        if ex.to.trim().is_empty() || !slug.is_match(&ex.to) {
            return Err(DomainError::Validation {
                field: "exit",
                message: format!("invalid exit target '{}'", ex.to),
            });
        }
    }

    Ok(())
}

fn validate_lua_for_room(room: &RoomYaml) -> AppResult<()> {
    let lua = Lua::new();

    for (hook, code) in &room.scripts.0 {
        compile_lua_chunk(&lua, &format!("room:{}:script:{:?}", room.id, hook), code)?;
    }

    // Inline object `use` blocks
    for obj in &room.objects {
        if let Some(code) = obj.use_.as_deref() {
            compile_lua_chunk(&lua, &format!("room:{}:object:{}:use", room.id, obj.id), code)?;
        }
    }

    Ok(())
}

fn check_lua_string(name: &str, code: &str) -> AppResult<()> {
    let bytes = code.as_bytes();
    if bytes.len() > MAX_LUA_BYTES {
        return Err(DomainError::Validation {
            field: "lua",
            message: format!("Lua chunk '{}' too large ({} bytes)", name, bytes.len()),
        });
    }
    let lower = code.to_ascii_lowercase();
    for tok in FORBIDDEN_LUA_TOKENS {
        if lower.contains(&tok.to_ascii_lowercase()) {
            return Err(DomainError::Validation {
                field: "lua",
                message: format!("Lua chunk '{}' contains forbidden token '{}'", name, tok),
            });
        }
    }
    Ok(())
}

fn compile_lua_chunk(lua: &Lua, name: &str, code: &str) -> AppResult<()> {
    check_lua_string(name, code)?;
    lua.load(code).set_name(name).into_function()?;
    Ok(())
}
