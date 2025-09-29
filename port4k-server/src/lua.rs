use anyhow::{Context, Result};
use mlua::{Lua, Function, Value, Table};
use std::time::Duration;
use mlua::prelude::LuaError;
use tokio::sync::{mpsc, oneshot};
use crate::db::Db;

pub enum LuaJob {
    OnEnter { reply: oneshot::Sender<Result<()>> },
    OnCommandPlaytest { reply: oneshot::Sender<Result<String>> }
}

pub fn start_lua_worker() -> mpsc::Sender<LuaJob> {
    let (tx, mut rx) = mpsc::channel::<LuaJob>(64);

    std::thread::spawn(move || {
        let lua = mlua::Lua::new(); // stays on this thread forever
        while let Some(job) = rx.blocking_recv() {
            match job {
                LuaJob::OnEnter { reply } => {
                    let res = (|| -> Result<()> {
                        if let Ok(f) = lua.globals().get::<_, mlua::Function>("on_enter") {
                            f.call::<_, ()>(())?;
                        }
                        Ok(())
                    })();
                    let _ = reply.send(res);
                }
                LuaJob::OnCommandPlaytest { reply } => {
                    let res = (|| -> Result<String> {
                        if let Ok(f) = lua.globals().get::<_, mlua::Function>("on_command") {
                            f.call::<_, ()>(())?;
                        }
                        Ok("".into())
                    })();
                    let _ = reply.send(res);
                }
            }
        }
    });

    tx
}

/// Run `on_command` if present for a blueprint room. Returns:
/// - Ok(Some(text)) if handled (accumulated output)
/// - Ok(None) if no handler or not handled
async fn run_on_command_playtest(
    db: &Db,
    bp: &str,
    room: &str,
    account: &str,
    verb: &str,
    args: &[String],
) -> Result<Option<String>> {
    let bp_owned = bp.to_string();
    let room_owned = room.to_string();
    let account_owned = account.to_string();

    let src = match db.bp_script_get_live(bp, room, "on_command").await? {
        Some(s) => s,
        None => return Ok(None),
    };

    // Prepare Lua VM
    let lua = Lua::new(); // minimal libs
    let out = std::sync::Arc::new(parking_lot::Mutex::new(String::new()));

    // Host API
    {
        let globals = lua.globals();
        let out_send = out.clone();
        let send = lua.create_function(move |_, (text,): (String,)| {
            out_send.lock().push_str(&format!("{text}\n"));
            Ok(())
        })?;
        globals.set("send", send)?;

        let out_b = out.clone();
        let broadcast_room = lua.create_function(move |_, (text,): (String,)| {
            out_b.lock().push_str(&format!("{text}\n"));
            Ok(())
        })?;
        globals.set("broadcast_room", broadcast_room)?;

        // get/set_state (room)
        let db_get_state = db.clone();
        let bp1 = bp_owned.clone();
        let room1 = room_owned.clone();

        let get_state = lua.create_function(move |lua_ctx, (key,): (String,)| {
            let db = db_get_state.clone();
            let bp = bp1.clone();
            let room = room1.clone();

            let val = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    db.bp_room_kv_get(&bp, &room, &key).await
                })
            }).map_err(mlua::Error::external)?;
            Ok(match val {
                Some(v) => serde_json_to_lua(lua_ctx, v)?,
                None => Value::Nil,
            })
        })?;
        lua.globals().set("get_state", get_state)?;

        let db_set_state = db.clone();
        let bp2 = bp_owned.clone();
        let room2 = room_owned.clone();

        let set_state = lua.create_function(move |lua_ctx, (key, value): (String, Value)| {
            let v = lua_to_serde_json(lua_ctx, value)?;
            let db = db_set_state.clone();
            let bp = bp2.clone();
            let room = room2.clone();

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    db.bp_room_kv_set(&bp, &room, &key, &v).await
                })
            }).map_err(mlua::Error::external)?;
            Ok(())
        })?;
        lua.globals().set("set_state", set_state)?;

        // get/set_player (per player, scoped to this room)
        let db_get_player = db.clone();
        let bp3 = bp_owned.clone();
        let room3 = room_owned.clone();
        let acc1 = account_owned.clone();

        let get_player = lua.create_function(move |lua_ctx, (key,): (String,)| {
            let db = db_get_player.clone();
            let bp = bp3.clone();
            let room = room3.clone();
            let acc = acc1.clone();

            let val = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    db.bp_player_kv_get(&bp, &acc, &room, &key).await
                })
            }).map_err(|e| LuaError::external(e))?;
            Ok(match val {
                Some(v) => serde_json_to_lua(&lua_ctx, v)?,
                None => Value::Nil,
            })
        })?;
        lua.globals().set("get_player", get_player)?;

        let db_set_player = db.clone();
        let bp4 = bp_owned.clone();
        let room4 = room_owned.clone();
        let acc2 = account_owned.clone();

        let set_player = lua.create_function(move |lua_ctx, (key, value): (String, Value)| {
            let v = lua_to_serde_json(lua_ctx, value)?;
            let db = db_set_player.clone();
            let bp = bp4.clone();
            let room = room4.clone();
            let acc = acc2.clone();

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    db.bp_player_kv_set(&bp, &acc, &room, &key, &v).await
                })
            }).map_err(|e| LuaError::external(e))?;
            Ok(())
        })?;
        lua.globals().set("set_player", set_player)?;
    }

    // Load script & resolve function
    let chunk = lua.load(&src);
    chunk
        .set_name(&format!("{bp}:{room}:on_command"))
        .exec()?;

    // Call handler
    let handler: Option<Function> = match lua.globals().get("on_command") {
        Ok(f) => Some(f),
        Err(_) => None,
    };
    if let Some(func) = handler {
        // Convert args: Vec<String> -> Lua table
        let t: Table = lua.create_table()?;
        for (i, a) in args.iter().enumerate() { t.set(i + 1, a.as_str())?; }

        // Run with a simple overall timeout (coarse)
        let lua_call = async {
            func.call::<_, Value>((verb, t))
        };
        tokio::time::timeout(Duration::from_millis(50), lua_call).await
            .map_err(|_| anyhow::anyhow!("script timeout"))?
            .context("on_command failed")?;

        let text = out.lock().clone();
        return Ok(Some(text));
    }

    Ok(None)
}

/// Optional: fire on_enter when entering rooms in playtest
pub async fn run_on_enter_playtest(db: &Db, bp: &str, room: &str, account: &str) -> Result<Option<String>> {
    let src = match db.bp_script_get_live(bp, room, "on_enter").await? {
        Some(s) => s,
        None => return Ok(None),
    };
    let lua = Lua::new();
    let out = std::sync::Arc::new(parking_lot::Mutex::new(String::new()));

    // send/broadcast_room only (reuse helpers above)
    {
        let g = lua.globals();
        let o = out.clone();
        g.set("send", lua.create_function(move |_, (t,): (String,)| {
            o.lock().push_str(&t); o.lock().push('\n'); Ok(())
        })?)?;
        let o2 = out.clone();
        g.set("broadcast_room", lua.create_function(move |_, (t,): (String,)| {
            o2.lock().push_str(&t); o2.lock().push('\n'); Ok(())
        })?)?;
    }

    lua.load(&src)
        .set_name(&format!("{bp}:{room}:on_enter"))
        .exec()?;
    if let Ok(f) = lua.globals().get::<_, Function>("on_enter") {
        tokio::time::timeout(Duration::from_millis(50), async {
            f.call::<_, ()>((account.to_string(),))
        }).await.map_err(|_| anyhow::anyhow!("script timeout"))??;
    }

    Ok(Some(out.lock().clone()))
}

fn serde_json_to_lua(lua: &Lua, v: serde_json::Value) -> mlua::Result<Value> {
    Ok(match v {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Boolean(b),
        serde_json::Value::Number(n) => {
            Value::Number(n.as_f64().unwrap_or(0.0))
        }
        serde_json::Value::String(s) => Value::String(lua.create_string(&s)?),
        serde_json::Value::Array(arr) => {
            let t = lua.create_table()?;
            for (i, el) in arr.into_iter().enumerate() {
                t.set(i + 1, serde_json_to_lua(lua, el)?)?;
            }
            Value::Table(t)
        }
        serde_json::Value::Object(map) => {
            let t = lua.create_table()?;
            for (k, el) in map.into_iter() {
                t.set(k, serde_json_to_lua(lua, el)?)?;
            }
            Value::Table(t)
        }
    })
}

fn lua_to_serde_json(lua: &Lua, v: Value) -> mlua::Result<serde_json::Value> {
    Ok(match v {
        Value::Nil => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(b),
        Value::Number(n) => serde_json::Value::Number(
            serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0))
        ),
        Value::String(s) => serde_json::Value::String(s.to_str()?.to_string()),
        Value::Table(t) => {
            // Heuristic: numeric keys -> array; otherwise object
            let mut is_array = true;
            let mut max_index = 0usize;
            for pair in t.clone().pairs::<Value, Value>() {
                if let Ok((Value::Number(k), _)) = pair {
                    if let Some(i) = (k as f64).round().to_string().parse::<usize>().ok() {
                        if i > max_index { max_index = i; }
                    } else { is_array = false; break; }
                } else { is_array = false; break; }
            }
            if is_array {
                let mut vec = Vec::with_capacity(max_index);
                for i in 1..=max_index {
                    vec.push(lua_to_serde_json(lua, t.get(i)? )?);
                }
                serde_json::Value::Array(vec)
            } else {
                let mut map = serde_json::Map::new();
                for pair in t.pairs::<String, Value>() {
                    let (k, vv) = pair?;
                    map.insert(k, lua_to_serde_json(lua, vv)?);
                }
                serde_json::Value::Object(map)
            }
        }
        _ => serde_json::Value::Null,
    })
}
