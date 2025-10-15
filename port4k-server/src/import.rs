use crate::db::error::DbError;
use crate::error::{AppResult, DomainError, InfraError};
use crate::hardening::{ALLOWED_DIRS, FORBIDDEN_LUA_TOKENS, MAX_LUA_BYTES};
use crate::models::types::BlueprintId;
use crate::util::{list_yaml_files_guarded, resolve_content_subdir};
use mlua::Lua;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::{fs, path::Path};
use tokio_postgres::Transaction;

#[derive(Debug, Deserialize)]
pub struct RoomYaml {
    pub id: String,   // "entry_hall"
    pub name: String, // "Entry Hall"
    #[serde(default)]
    pub short: Option<String>, // oneliner
    #[serde(rename = "description")]
    pub full_desc: String, // long text
    #[serde(default)]
    pub hints: Vec<String>,
    #[serde(default)]
    pub objects: Vec<ObjectYaml>,
    #[serde(default)]
    pub exits: Vec<ExitYaml>,
    #[serde(default)]
    pub scripts: ScriptsYaml,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectYaml {
    pub id: String,
    #[serde(default)]
    pub nouns: Vec<String>,
    pub short: String,
    pub description: String,
    #[serde(default)]
    pub examine: Option<String>,
    #[serde(default)]
    pub state: serde_json::Value, // arbitrary map
    #[serde(default)]
    pub loot: Option<serde_json::Value>,
    #[serde(default)]
    pub use_: Option<String>, // Lua (key "use" in YAML)
    #[serde(rename = "use", default)]
    pub _use_compat: Option<String>, // compat alias
}

#[derive(Debug, Deserialize)]
pub struct ExitYaml {
    pub dir: String, // "north"
    pub to: String,  // "hallway_1"
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub locked: Option<bool>,
    #[serde(default)]
    pub visible_when_locked: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ScriptsYaml {
    #[serde(default)]
    pub on_enter: Option<String>, // Lua
    #[serde(default)]
    pub on_command: Option<String>, // Lua
    #[serde(default)]
    pub objects: HashMap<String, ScriptObjectHandlers>, // id -> handlers
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ScriptObjectHandlers {
    #[serde(default)]
    pub use_: Option<String>, // Lua
    #[serde(rename = "use", default)]
    pub _use_compat: Option<String>, // compat alias
}

pub async fn import_blueprint_subdir(
    blueprint_id: BlueprintId,
    subdir: &str,
    content_base: &Path,
    db: &crate::db::Db,
) -> AppResult<()> {
    let dir = resolve_content_subdir(content_base, subdir)?;
    let files = list_yaml_files_guarded(&dir)?;

    let mut client = db.pool.get().await.map_err(DbError::from)?;
    let tx = client.build_transaction().start().await.map_err(DbError::from)?;

    // Process sequentially for clear error reporting (can be parallelized if needed)
    for path in files {
        let text = fs::read_to_string(&path).map_err(InfraError::from)?;
        let mut room: RoomYaml = serde_yaml::from_str(&text)?;

        // normalize "use"
        for o in &mut room.objects {
            if o.use_.is_none() {
                o.use_ = o._use_compat.take();
            }
        }
        for h in room.scripts.objects.values_mut() {
            if h.use_.is_none() {
                h.use_ = h._use_compat.take();
            }
        }

        validate_room_semantics(&room)?;

        // Note we don't await here
        validate_lua_for_room(&room)?;

        upsert_room_and_exits(blueprint_id, &room, &tx).await?;
    }

    tx.commit().await.map_err(DbError::from)?;
    Ok(())
}

async fn upsert_room_and_exits(bp_id: BlueprintId, r: &RoomYaml, tx: &Transaction<'_>) -> AppResult<()> {
    let title = &r.name;
    let short = r.short.as_deref().unwrap_or_default();
    let body = &r.full_desc;

    // Store objects & scripts as JSONB
    let objects_json = serde_json::to_value(&r.objects)?;
    let scripts_json = json!({
        "on_enter":   r.scripts.on_enter,
        "on_command": r.scripts.on_command,
        "objects":    r.scripts.objects, // map id -> { use: "<lua>" }
    });
    let hints_json = serde_json::to_value(&r.hints)?;

    // UPSERT room
    tx.execute(
        r#"
        INSERT INTO bp_rooms (bp_id, key, title, short, body, hints, objects, scripts)
        VALUES ($1,$2,$3,$4,$5,$6::jsonb,$7::jsonb,$8::jsonb)
        ON CONFLICT (bp_id, key) DO UPDATE
        SET title  = EXCLUDED.title,
            short  = EXCLUDED.short,
            body   = EXCLUDED.body,
            hints  = EXCLUDED.hints,
            objects= EXCLUDED.objects,
            scripts= EXCLUDED.scripts
        "#,
        &[
            &bp_id,
            &r.id,
            &title,
            &short,
            &body,
            &hints_json,
            &objects_json,
            &scripts_json,
        ],
    )
    .await
    .map_err(DbError::from)?;

    // Upsert exits: we’ll do simple merge (insert or update desc/locked/visible)
    for ex in &r.exits {
        tx.execute(
            r#"
            INSERT INTO bp_exits (bp_id, from_key, dir, to_key, locked, description, visible_when_locked)
            VALUES ($1,$2,$3,$4, COALESCE($5,false), $6, COALESCE($7,true))
            ON CONFLICT (bp_id, from_key, dir) DO UPDATE
            SET to_key = EXCLUDED.to_key,
                locked  = EXCLUDED.locked,
                description  = EXCLUDED.description,
                visible_when_locked = EXCLUDED.visible_when_locked
            "#,
            &[
                &bp_id,
                &r.id,
                &ex.dir.to_ascii_lowercase(),
                &ex.to,
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

fn validate_lua_for_room(room: &RoomYaml) -> AppResult<()> {
    // Fresh state per room (no std libs loaded by default)
    let lua = Lua::new();

    if let Some(code) = room.scripts.on_enter.as_deref() {
        compile_lua_chunk(&lua, &format!("room:{}:on_enter", room.id), code)?;
    }
    if let Some(code) = room.scripts.on_command.as_deref() {
        compile_lua_chunk(&lua, &format!("room:{}:on_command", room.id), code)?;
    }

    // Object script handlers under scripts.objects
    for (obj_id, h) in &room.scripts.objects {
        if let Some(code) = h.use_.as_deref() {
            compile_lua_chunk(&lua, &format!("room:{}:object:{}:use", room.id, obj_id), code)?;
        }
    }

    // Inline object `use` blocks in the objects list
    for obj in &room.objects {
        if let Some(code) = obj.use_.as_deref() {
            compile_lua_chunk(&lua, &format!("room:{}:object:{}:use(inline)", room.id, obj.id), code)?;
        }
    }

    Ok(())
}

pub fn validate_room_semantics(room: &RoomYaml) -> AppResult<()> {
    // id/name/desc basics
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

    // unique object ids
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
    }

    // {obj:ID} placeholders must reference existing objects
    let re = Regex::new(r"\{obj:([a-zA-Z0-9_\-]+)}").unwrap();
    for cap in re.captures_iter(&room.full_desc) {
        let id = cap[1].to_string();
        if !ids.contains(&id) {
            return Err(DomainError::Validation {
                field: "description",
                message: "description references unknown object id: {}".into(),
            });
        }
    }

    // exits: dir whitelist, to-format (slug-ish)
    let slug = Regex::new(r"^[a-zA-Z0-9_\-:]+$").unwrap(); // allow bp:room or plain id
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

fn check_lua_string(name: &str, code: &str) -> AppResult<()> {
    let bytes = code.as_bytes();
    if bytes.len() > MAX_LUA_BYTES {
        return Err(DomainError::Validation {
            field: "exit",
            message: format!("Lua chunk '{}' too large ({} bytes)", name, bytes.len()),
        });
    }
    let lower = code.to_ascii_lowercase();
    for tok in FORBIDDEN_LUA_TOKENS {
        if lower.contains(&tok.to_ascii_lowercase()) {
            return Err(DomainError::Validation {
                field: "exit",
                message: format!("Lua chunk '{}' contains forbidden token '{}'", name, tok),
            });
        }
    }
    Ok(())
}

fn compile_lua_chunk(lua: &Lua, name: &str, code: &str) -> AppResult<()> {
    check_lua_string(name, code)?;

    lua.load(code)
        .set_name(name)
        // .context(format!("setting name for chunk {}", name))?
        .into_function()?;
    Ok(())
}
