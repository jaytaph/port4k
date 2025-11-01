use crate::db::error::DbError;
use crate::error::{AppResult, DomainError, InfraError};
use crate::hardening::{ALLOWED_DIRS, FORBIDDEN_LUA_TOKENS, MAX_LUA_BYTES};
use crate::lua::ScriptHook;
use crate::models::types::BlueprintId;
use crate::util::{list_yaml_files_guarded, resolve_content_subdir};
use mlua::Lua;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::{fs, path::Path};
use tokio_postgres::Transaction;

// ====== v5 YAML models ======

#[derive(Debug, Deserialize)]
struct RoomYaml {
    pub version: u8,  // must be 5
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
    #[serde(default)]
    pub items_catalog: Vec<ItemCatalogYaml>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ItemCatalogYaml {
    pub id: String,
    pub name: String,
    pub nouns: Vec<String>,
    pub short: String,
    pub description: String,
    #[serde(default)]
    pub examine: Option<String>,
    pub stackable: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct HintYaml {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub when: Option<String>, // "enter" | "first_look" | "manual" | ...
    #[serde(default)]
    pub cooldown: Option<i32>,
    #[serde(default)]
    pub once: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
struct FlagsYaml {
    pub hidden: Option<bool>,
    pub revealed: Option<bool>,
    pub takeable: Option<bool>,
    pub stackable: Option<bool>,
}

impl Default for FlagsYaml {
    fn default() -> Self {
        FlagsYaml {
            hidden: Some(false),    // Not hidden from user
            revealed: Some(true),   // The object is revealed
            takeable: Some(false),  // Not takeable by user
            stackable: Some(false), // Not stackable (multiple copies)
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct LootYaml {
    pub items: Vec<String>,
    #[serde(default)]
    pub credits: i32,
    #[serde(default = "default_true")]
    pub once: bool,
    #[serde(default)]
    pub shared: bool, // NEW: false = per-player, true = global
}

fn default_true() -> bool {
    true
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
    pub flags: Option<FlagsYaml>,
    #[serde(default)]
    pub state: HashMap<String, serde_json::Value>, // arbitrary map (revealed, etc)
    #[serde(default)]
    pub controls: Vec<String>, // ["exit:north.locked","object:door.locked"]

    #[serde(default)]
    pub loot: Option<LootYaml>,

    #[serde(default)]
    pub on_use_: Option<String>, // Lua (key "on_use" in YAML)
    #[serde(rename = "on_use", default)]
    pub _on_use_compat: Option<String>, // compat alias
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

#[derive(Debug, Serialize, Deserialize, Default)]
struct ScriptYaml(HashMap<ScriptHook, String>);

// ====== Entry point ======

pub async fn import_blueprint_sub_dir(
    blueprint_id: BlueprintId,
    sub_dir: &str,
    content_base: &Path,
    db: &crate::db::Db,
) -> AppResult<()> {
    println!("🚀 Starting blueprint import for '{}'", sub_dir);

    let dir = resolve_content_subdir(content_base, sub_dir)?;
    println!("📁 Scanning directory: {}", dir.display());

    let files = list_yaml_files_guarded(&dir)?;
    println!("📄 Found {} YAML file(s)", files.len());

    // Parse first
    let mut rooms: Vec<RoomYaml> = Vec::new();
    for (idx, path) in files.iter().enumerate() {
        println!("\n[{}/{}] Parsing: {}", idx + 1, files.len(), path.display());

        let text = fs::read_to_string(path).map_err(InfraError::from)?;
        let mut room: RoomYaml = serde_yaml::from_str(&text)?;

        // normalize "on_use"
        for o in &mut room.objects {
            if o.on_use_.is_none() {
                o.on_use_ = o._on_use_compat.take();
            }
        }

        println!("  ✓ Room: '{}' (id: {})", room.name, room.id);
        println!("    • {} object(s)", room.objects.len());
        println!("    • {} exit(s)", room.exits.len());
        println!("    • {} hint(s)", room.hints.len());
        println!("    • {} script hook(s)", room.scripts.0.len());
        println!("    • {} item(s) in catalog", room.items_catalog.len());

        print!("  🔍 Validating semantics...");
        validate_room_semantics(&room)?;
        println!(" ✓");

        print!("  🔧 Compiling Lua scripts...");
        validate_lua_for_room(&room)?;
        println!(" ✓");

        rooms.push(room);
    }

    // NEW: Collect all items from all rooms in this blueprint
    println!("\n📦 Collecting items catalog from all rooms...");
    let mut all_items: HashMap<String, ItemCatalogYaml> = HashMap::new();

    for room in &rooms {
        for item in &room.items_catalog {
            if let Some(existing) = all_items.get(&item.id) {
                // Verify consistency: same item_key must have identical definition
                if existing.name != item.name
                    || existing.short != item.short
                    || existing.description != item.description
                    || existing.stackable != item.stackable
                {
                    return Err(DomainError::Validation {
                        field: "items_catalog",
                        message: format!(
                            "Item '{}' has inconsistent definitions across rooms. All definitions must match.",
                            item.id
                        ),
                    });
                }
                // Verify nouns match
                let mut existing_nouns = existing.nouns.clone();
                let mut item_nouns = item.nouns.clone();
                existing_nouns.sort();
                item_nouns.sort();
                if existing_nouns != item_nouns {
                    return Err(DomainError::Validation {
                        field: "items_catalog",
                        message: format!(
                            "Item '{}' has different nouns across rooms. All definitions must match.",
                            item.id
                        ),
                    });
                }
            } else {
                all_items.insert(item.id.clone(), item.clone());
            }
        }
    }

    println!("  ✓ Found {} unique item(s) across all rooms", all_items.len());

    println!("\n💾 Starting database transaction...");
    let mut client = db.pool.get().await.map_err(DbError::from)?;
    let tx = client.build_transaction().start().await.map_err(DbError::from)?;

    // Pass 1: upsert rooms
    println!("\n📝 Pass 1: Creating room headers...");
    let mut room_ids: HashMap<String, uuid::Uuid> = HashMap::new();
    for (idx, r) in rooms.iter().enumerate() {
        print!("  [{}/{}] Upserting room '{}'...", idx + 1, rooms.len(), r.id);
        let room_id = upsert_room_header(&tx, blueprint_id, r).await?;
        room_ids.insert(r.id.clone(), room_id);
        println!(" ✓ ({})", room_id);
    }

    if !all_items.is_empty() {
        println!("\n📦 Pass 1b: Registering blueprint-level items catalog...");
        upsert_blueprint_items_catalog(&tx, blueprint_id, &all_items).await?;
        println!("  ✓ Registered {} item(s)", all_items.len());
    }

    // Pass 2: kv, objects, scripts, items_catalog
    println!("\n🔧 Pass 2: Adding objects, items, state, and scripts...");
    for (idx, r) in rooms.iter().enumerate() {
        let room_id = *room_ids.get(&r.id).expect("room id present");
        println!("  [{}/{}] Processing room '{}'...", idx + 1, rooms.len(), r.id);

        if !r.state.is_empty() {
            print!("    • Upserting {} state key(s)...", r.state.len());
            upsert_room_kv(&tx, room_id, &r.state).await?;
            println!(" ✓");
        }

        if !r.objects.is_empty() {
            print!("    • Creating {} object(s)...", r.objects.len());
            upsert_objects(&tx, room_id, &r.objects).await?;
            println!(" ✓");
        }

        if !r.scripts.0.is_empty() {
            print!("    • Installing {} script hook(s)...", r.scripts.0.len());
            upsert_room_scripts(&tx, room_id, &r.scripts).await?;
            println!(" ✓");
        }
    }

    // Pass 3: exits
    println!("\n🚪 Pass 3: Linking exits...");
    for (idx, r) in rooms.iter().enumerate() {
        let from_room_id = *room_ids.get(&r.id).unwrap();
        if !r.exits.is_empty() {
            print!(
                "  [{}/{}] Creating {} exit(s) from '{}'...",
                idx + 1,
                rooms.len(),
                r.exits.len(),
                r.id
            );
            upsert_exits(&tx, from_room_id, &r.exits, &room_ids).await?;
            println!(" ✓");
        }
    }

    println!("\n💾 Committing transaction...");
    tx.commit().await.map_err(DbError::from)?;

    println!("✨ Import complete! {} room(s) successfully imported.\n", rooms.len());
    Ok(())
}

// ====== DB writers ======

async fn upsert_room_header(tx: &Transaction<'_>, bp_id: BlueprintId, r: &RoomYaml) -> AppResult<uuid::Uuid> {
    let title = &r.name;
    let short = r.short.as_deref().unwrap_or_default();
    let body = &r.full_desc;

    // Store hints as JSON (structured v3)
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
        let flags_json = serde_json::to_value(o.flags.as_ref().unwrap_or(&FlagsYaml::default()))?;
        let controls_json = serde_json::to_value(&o.controls)?;
        let loot_json = serde_json::to_value(&o.loot)?;

        let row = tx
            .query_one(
                r#"
                INSERT INTO bp_objects
                    (room_id, name, short, description, examine, use_lua,
                    position, flags, controls, loot)
                VALUES
                    ($1,$2,$3,$4,$5,$6,$7,$8::jsonb,$9::jsonb,$10::jsonb)
                RETURNING id
                "#,
                &[
                    &room_id,
                    &o.id,
                    &o.short,
                    &o.description,
                    &o.examine,
                    &o.on_use_,
                    &(pos as i32),
                    &flags_json,
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

async fn upsert_blueprint_items_catalog(
    tx: &Transaction<'_>,
    bp_id: BlueprintId,
    items: &HashMap<String, ItemCatalogYaml>,
) -> AppResult<()> {
    // Delete existing items for this blueprint
    tx.execute("DELETE FROM bp_item_nouns WHERE bp_id = $1", &[&bp_id])
        .await
        .map_err(DbError::from)?;
    tx.execute("DELETE FROM bp_items_catalog WHERE bp_id = $1", &[&bp_id])
        .await
        .map_err(DbError::from)?;

    // Insert all items
    for item in items.values() {
        let row = tx
            .query_one(
                r#"
                INSERT INTO bp_items_catalog
                    (bp_id, item_key, name, short, description, examine, stackable)
                VALUES
                    ($1, $2, $3, $4, $5, $6, $7)
                RETURNING id
                "#,
                &[
                    &bp_id,
                    &item.id,
                    &item.name,
                    &item.short,
                    &item.description,
                    &item.examine,
                    &item.stackable,
                ],
            )
            .await
            .map_err(DbError::from)?;
        let item_id: uuid::Uuid = row.get(0);

        // Insert nouns for this item
        for noun in &item.nouns {
            tx.execute(
                r#"
                INSERT INTO bp_item_nouns (bp_id, item_id, noun)
                VALUES ($1, $2, $3)
                "#,
                &[&bp_id, &item_id, noun],
            )
            .await
            .map_err(DbError::from)?;
        }
    }

    Ok(())
}

async fn upsert_room_scripts(tx: &Transaction<'_>, room_id: uuid::Uuid, scripts: &ScriptYaml) -> AppResult<()> {
    // single-row table keyed by room_id
    for (hook, script) in scripts.0.iter() {
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
    if room.version != 5 {
        return Err(DomainError::Validation {
            field: "room.version",
            message: "unsupported room schema version; expected 5".into(),
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

    // Validate items_catalog
    let mut item_ids = HashSet::new();
    let mut item_nouns = HashSet::new();
    for item in &room.items_catalog {
        if item.id.trim().is_empty() {
            return Err(DomainError::Validation {
                field: "items_catalog",
                message: "item with empty id".into(),
            });
        }
        if !item_ids.insert(&item.id) {
            return Err(DomainError::Validation {
                field: "items_catalog",
                message: format!("duplicate item id: {}", item.id),
            });
        }
        if item.name.trim().is_empty() {
            return Err(DomainError::Validation {
                field: "items_catalog",
                message: format!("item '{}' has empty name", item.id),
            });
        }
        if item.nouns.is_empty() {
            return Err(DomainError::Validation {
                field: "items_catalog",
                message: format!("item '{}' has no nouns", item.id),
            });
        }
        for noun in &item.nouns {
            if noun.trim().is_empty() {
                return Err(DomainError::Validation {
                    field: "items_catalog",
                    message: format!("item '{}' has empty noun", item.id),
                });
            }
            if !item_nouns.insert(noun) {
                return Err(DomainError::Validation {
                    field: "items_catalog",
                    message: format!("duplicate item noun: {}", noun),
                });
            }
        }
        if item.short.trim().is_empty() {
            return Err(DomainError::Validation {
                field: "items_catalog",
                message: format!("item '{}' has empty short description", item.id),
            });
        }
        if item.description.trim().is_empty() {
            return Err(DomainError::Validation {
                field: "items_catalog",
                message: format!("item '{}' has empty description", item.id),
            });
        }
    }

    // Validate that loot references valid items from catalog
    for obj in &room.objects {
        if let Some(loot) = &obj.loot {
            for item_id in &loot.items {
                if !item_ids.contains(item_id) {
                    return Err(DomainError::Validation {
                        field: "object.loot",
                        message: format!(
                            "Object '{}' references item '{}' in loot, but this item is not defined in items_catalog",
                            obj.id, item_id
                        ),
                    });
                }
            }
        }
    }

    // object ids unique
    let mut obj_ids = HashSet::new();
    for o in &room.objects {
        if o.id.trim().is_empty() {
            return Err(DomainError::Validation {
                field: "object",
                message: "object with empty id".into(),
            });
        }
        if !obj_ids.insert(&o.id) {
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
            if !obj_ids.contains(&id) {
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
        if let Some(code) = obj.on_use_.as_deref() {
            compile_lua_chunk(&lua, &format!("room:{}:object:{}:on_use", room.id, obj.id), code)?;
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
