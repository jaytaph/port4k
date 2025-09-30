use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::{fs, path::Path};
use mlua::Lua;
use tokio_postgres::Transaction;
use regex::Regex;
use crate::hardering::{ALLOWED_DIRS, FORBIDDEN_LUA_TOKENS, MAX_LUA_BYTES};
use crate::util::{list_yaml_files_guarded, resolve_content_subdir};

#[derive(Debug, Deserialize)]
pub struct RoomYaml {
    pub id: String,              // "entry_hall"
    pub name: String,            // "Entry Hall"
    #[serde(default)]
    pub short: Option<String>,   // oneliner
    #[serde(rename = "description")]
    pub full_desc: String,       // long text
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
    pub state: serde_json::Value,      // arbitrary map
    #[serde(default)]
    pub loot: Option<serde_json::Value>,
    #[serde(default)]
    pub use_: Option<String>,          // Lua (key "use" in YAML)
    #[serde(rename = "use", default)]
    pub _use_compat: Option<String>,   // compat alias
}

#[derive(Debug, Deserialize)]
pub struct ExitYaml {
    pub dir: String,            // "north"
    pub to: String,             // "hallway_1"
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
    pub on_enter: Option<String>,     // Lua
    #[serde(default)]
    pub on_command: Option<String>,   // Lua
    #[serde(default)]
    pub objects: HashMap<String, ScriptObjectHandlers>, // id -> handlers
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ScriptObjectHandlers {
    #[serde(default)]
    pub use_: Option<String>,         // Lua
    #[serde(rename = "use", default)]
    pub _use_compat: Option<String>,  // compat alias
}

pub async fn import_blueprint_subdir(
    bp: &str,
    subdir: &str,
    content_base: &Path,
    db: &crate::db::Db,
) -> Result<()> {
    let dir = resolve_content_subdir(content_base, subdir)?;
    let files = list_yaml_files_guarded(&dir)?;

    let mut client = db.pool.get().await?;
    let tx = client.build_transaction().start().await?;

    // Process sequentially for clear error reporting (can be parallelized if needed)
    for path in files {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut room: RoomYaml = serde_yaml::from_str(&text)
            .with_context(|| format!("parsing YAML {}", path.display()))?;

        // normalize "use"
        for o in &mut room.objects {
            if o.use_.is_none() { o.use_ = o._use_compat.take(); }
        }
        for (_, h) in &mut room.scripts.objects {
            if h.use_.is_none() { h.use_ = h._use_compat.take(); }
        }

        validate_room_semantics(&room)
            .with_context(|| format!("schema validation failed for {}", room.id))?;

        // Note we don't await here
        validate_lua_for_room(&room)
            .with_context(|| format!("Lua validation failed for {}", room.id))?;

        upsert_room_and_exits(bp, &room, &tx).await
            .with_context(|| format!("upserting room {}", room.id))?;
    }

    tx.commit().await?;
    Ok(())
}

async fn upsert_room_and_exits(bp: &str, r: &RoomYaml, tx: &Transaction<'_>) -> Result<()> {
    let title = &r.name;
    let short = r.short.as_deref().unwrap_or_default();
    let body  = &r.full_desc;

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
        INSERT INTO bp_rooms (bp, room, title, short, body, hints, objects, scripts)
        VALUES ($1,$2,$3,$4,$5,$6::jsonb,$7::jsonb,$8::jsonb)
        ON CONFLICT (bp, room) DO UPDATE
        SET title  = EXCLUDED.title,
            short  = EXCLUDED.short,
            body   = EXCLUDED.body,
            hints  = EXCLUDED.hints,
            objects= EXCLUDED.objects,
            scripts= EXCLUDED.scripts
        "#,
        &[&bp, &r.id, &title, &short, &body, &hints_json, &objects_json, &scripts_json]
    ).await?;

    // Upsert exits: we’ll do simple merge (insert or update desc/locked/visible)
    for ex in &r.exits {
        tx.execute(
            r#"
            INSERT INTO bp_exits (bp, from_room, dir, to_room, locked, "desc", visible_when_locked)
            VALUES ($1,$2,$3,$4, COALESCE($5,false), $6, COALESCE($7,true))
            ON CONFLICT (bp, from_room, dir) DO UPDATE
            SET to_room = EXCLUDED.to_room,
                locked  = EXCLUDED.locked,
                "desc"  = EXCLUDED."desc",
                visible_when_locked = EXCLUDED.visible_when_locked
            "#,
            &[&bp, &r.id, &ex.dir.to_ascii_lowercase(), &ex.to, &ex.locked, &ex.description, &ex.visible_when_locked]
        ).await?;
    }

    Ok(())
}


fn validate_lua_for_room(room: &RoomYaml) -> Result<()> {
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

pub fn validate_room_semantics(room: &RoomYaml) -> Result<()> {
    // id/name/desc basics
    if room.id.trim().is_empty() { bail!("room id empty"); }
    if room.name.trim().is_empty() { bail!("room name empty"); }
    if room.full_desc.trim().is_empty() { bail!("room desc empty"); }
    if room.id.len() > 64 { bail!("room id too long"); }
    if room.name.len() > 128 { bail!("room name too long"); }

    // unique object ids
    let mut ids = HashSet::new();
    for o in &room.objects {
        if o.id.trim().is_empty() { bail!("object with empty id"); }
        if !ids.insert(&o.id) { bail!("duplicate object id: {}", o.id); }
    }

    // {obj:ID} placeholders must reference existing objects
    let re = Regex::new(r"\{obj:([a-zA-Z0-9_\-]+)\}").unwrap();
    for cap in re.captures_iter(&room.full_desc) {
        let id = cap[1].to_string();
        if !ids.contains(&id) {
            bail!("description references unknown object id: {}", id);
        }
    }

    // exits: dir whitelist, to-format (slug-ish)
    let slug = Regex::new(r"^[a-zA-Z0-9_\-:]+$").unwrap(); // allow bp:room or plain id
    for ex in &room.exits {
        let d = ex.dir.to_ascii_lowercase();
        if !ALLOWED_DIRS.contains(&d.as_str()) {
            bail!("invalid exit dir '{}'", d);
        }
        if ex.to.trim().is_empty() || !slug.is_match(&ex.to) {
            bail!("invalid exit target '{}'", ex.to);
        }
    }

    Ok(())
}


fn check_lua_string(name: &str, code: &str) -> Result<()> {
    let bytes = code.as_bytes();
    if bytes.len() > MAX_LUA_BYTES {
        bail!("Lua chunk '{}' too large ({} bytes)", name, bytes.len());
    }
    let lower = code.to_ascii_lowercase();
    for tok in FORBIDDEN_LUA_TOKENS {
        if lower.contains(&tok.to_ascii_lowercase()) {
            bail!("Lua chunk '{}' contains forbidden token '{}'", name, tok);
        }
    }
    Ok(())
}



fn compile_lua_chunk(lua: &Lua, name: &str, code: &str) -> Result<()> {
    check_lua_string(name, code)?;

    lua.load(code)
        .set_name(name)
        // .context(format!("setting name for chunk {}", name))?
        .into_function()
        .with_context(|| format!("Lua syntax error in {}", name))?;
    Ok(())
}
