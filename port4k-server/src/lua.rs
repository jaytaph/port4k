use anyhow::{Context, Result};
use mlua::{Function, Lua, Table, Value};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};

use crate::db::Db;

pub enum LuaJob {
    /// Called when a player enters a room
    OnEnter {
        reply: oneshot::Sender<Result<()>>,
    },
    /// Called when a player issues a command in a room
    OnCommand {
        db: Db,
        bp: String,
        room: String,
        account: String,
        verb: String,
        args: Vec<String>,
        reply: oneshot::Sender<Result<Option<String>>>,
    },
    // Called when a player issues a command in playtest mode (no DB state, just ephemeral)
    OnCommandPlaytest {
        db: Db,
        bp: String,
        room: String,
        account: String,
        verb: String,
        args: Vec<String>,
        reply: oneshot::Sender<Result<Option<String>>>,
    },
    // Called when we enter a room in playtest mode (no DB state, just ephemeral)
    OnEnterPlaytest {
        db: Db,
        bp: String,
        room: String,
        account: String,
        reply: oneshot::Sender<Result<Option<String>>>,
    }
}

/// Start a dedicated Lua worker thread with its own Lua state.
/// Pass the runtime `Handle` so the worker can run async DB calls with `handle.block_on(...)`.
pub fn start_lua_worker(rt_handle: Handle) -> mpsc::Sender<LuaJob> {
    let (tx, mut rx) = mpsc::channel::<LuaJob>(64);

    std::thread::spawn(move || {
        // One Lua VM for this thread
        let lua = Lua::new();

        while let Some(job) = rx.blocking_recv() {
            match job {
                LuaJob::OnEnter { reply } => {
                    let res = (|| -> Result<()> {
                        if let Ok(f) = lua.globals().get::<_, Function>("on_enter") {
                            f.call::<_, ()>(())?;
                        }
                        Ok(())
                    })();
                    let _ = reply.send(res);
                }

                LuaJob::OnCommand { .. } => {

                }

                LuaJob::OnCommandPlaytest { db, bp, room, account, verb, args, reply } => {
                    let res = (|| -> Result<Option<String>> {
                        // Load live script (async) via runtime handle
                        let src = rt_handle
                            .block_on(db.bp_script_get_live(&bp, &room, "on_command"))?
                            .unwrap_or_default();

                        if src.is_empty() {
                            return Ok(None);
                        }

                        // Output buffer
                        let out = std::sync::Arc::new(parking_lot::Mutex::new(String::new()));

                        // ----- Host API bindings -----
                        {
                            let globals = lua.globals();

                            // send(text)
                            {
                                let out_send = out.clone();
                                let send = lua.create_function(move |_, (text,): (String,)| {
                                    out_send.lock().push_str(&text);
                                    out_send.lock().push('\n');
                                    Ok(())
                                })?;
                                globals.set("send", send)?;
                            }

                            // broadcast_room(text) – here we just append to local out
                            {
                                let out_b = out.clone();
                                let broadcast_room = lua.create_function(
                                    move |_, (text,): (String,)| {
                                        out_b.lock().push_str(&text);
                                        out_b.lock().push('\n');
                                        Ok(())
                                    },
                                )?;
                                globals.set("broadcast_room", broadcast_room)?;
                            }

                            // get_state(key)
                            {
                                let db_cl = db.clone();
                                let bp_cl = bp.clone();
                                let room_cl = room.clone();
                                let handle = rt_handle.clone();
                                let get_state =
                                    lua.create_function(move |lua_ctx, (key,): (String,)| {
                                        let v = handle
                                            .block_on(db_cl.bp_room_kv_get(&bp_cl, &room_cl, &key))
                                            .map_err(mlua::Error::external)?;
                                        Ok(match v {
                                            Some(v) => serde_json_to_lua(lua_ctx, v)?,
                                            None => Value::Nil,
                                        })
                                    })?;
                                globals.set("get_state", get_state)?;
                            }

                            // set_state(key, value)
                            {
                                let db_cl = db.clone();
                                let bp_cl = bp.clone();
                                let room_cl = room.clone();
                                let handle = rt_handle.clone();
                                let set_state = lua.create_function(
                                    move |lua_ctx, (key, value): (String, Value)| {
                                        let v = lua_to_serde_json(lua_ctx, value)?;
                                        handle
                                            .block_on(db_cl.bp_room_kv_set(
                                                &bp_cl, &room_cl, &key, &v,
                                            ))
                                            .map_err(mlua::Error::external)?;
                                        Ok(())
                                    },
                                )?;
                                globals.set("set_state", set_state)?;
                            }

                            // get_player(key)
                            {
                                let db_cl = db.clone();
                                let bp_cl = bp.clone();
                                let room_cl = room.clone();
                                let acc_cl = account.clone();
                                let handle = rt_handle.clone();
                                let get_player =
                                    lua.create_function(move |lua_ctx, (key,): (String,)| {
                                        let v = handle
                                            .block_on(db_cl.bp_player_kv_get(
                                                &bp_cl, &acc_cl, &room_cl, &key,
                                            ))
                                            .map_err(mlua::Error::external)?;
                                        Ok(match v {
                                            Some(v) => serde_json_to_lua(lua_ctx, v)?,
                                            None => Value::Nil,
                                        })
                                    })?;
                                globals.set("get_player", get_player)?;
                            }

                            // set_player(key, value)
                            {
                                let db_cl = db.clone();
                                let bp_cl = bp.clone();
                                let room_cl = room.clone();
                                let acc_cl = account.clone();
                                let handle = rt_handle.clone();
                                let set_player = lua.create_function(
                                    move |lua_ctx, (key, value): (String, Value)| {
                                        let v = lua_to_serde_json(lua_ctx, value)?;
                                        handle
                                            .block_on(db_cl.bp_player_kv_set(
                                                &bp_cl, &acc_cl, &room_cl, &key, &v,
                                            ))
                                            .map_err(mlua::Error::external)?;
                                        Ok(())
                                    },
                                )?;
                                globals.set("set_player", set_player)?;
                            }
                        }

                        // ----- Load & run on_command(bp:room) -----
                        lua.load(&src)
                            .set_name(&format!("{bp}:{room}:on_command"))
                            .exec()?;

                        if let Ok(func) = lua.globals().get::<_, Function>("on_command") {
                            let t: Table = lua.create_table()?;
                            for (i, a) in args.iter().enumerate() {
                                t.set(i + 1, a.as_str())?;
                            }

                            // Synchronous call (keeps worker future Send)
                            func.call::<_, ()>((verb.as_str(), t))
                                .map_err(|e| anyhow::anyhow!(e))
                                .context("on_command failed")?;

                            let text = out.lock().clone();
                            return Ok(Some(text));
                        }

                        Ok(None)
                    })();

                    let _ = reply.send(res);
                }

                LuaJob::OnEnterPlaytest { db, bp, room, account, reply } => {
                    let res = (|| -> Result<Option<String>> {
                        let src = rt_handle
                            .block_on(db.bp_script_get_live(&bp, &room, "on_enter"))?
                            .unwrap_or_default();
                        if src.is_empty() {
                            return Ok(None);
                        }

                        let out = std::sync::Arc::new(parking_lot::Mutex::new(String::new()));

                        // Minimal host API: send / broadcast_room
                        {
                            let g = lua.globals();
                            let o = out.clone();
                            g.set("send", lua.create_function(move |_, (t,): (String,)| {
                                o.lock().push_str(&t);
                                o.lock().push('\n');
                                Ok(())
                            })?)?;
                            let o2 = out.clone();
                            g.set("broadcast_room", lua.create_function(move |_, (t,): (String,)| {
                                o2.lock().push_str(&t);
                                o2.lock().push('\n');
                                Ok(())
                            })?)?;
                        }

                        lua.load(&src).set_name(&format!("{bp}:{room}:on_enter")).exec()?;

                        if let Ok(f) = lua.globals().get::<_, Function>("on_enter") {
                            f.call::<_, ()>((account.as_str(),))
                                .map_err(|e| anyhow::anyhow!(e))
                                .context("on_enter failed")?;
                        }

                        Ok(Some(out.lock().clone()))
                    })();

                    let _ = reply.send(res);
                }
            }
        }
    });

    tx
}

// ---------- JSON <-> Lua helpers ----------

fn serde_json_to_lua(lua: &Lua, v: serde_json::Value) -> mlua::Result<Value<'_>> {
    Ok(match v {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Boolean(b),
        serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
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
            serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0)),
        ),
        Value::String(s) => serde_json::Value::String(s.to_str()?.to_string()),
        Value::Table(t) => {
            // Heuristic: numeric 1..N keys => array; else object
            let mut is_array = true;
            let mut max_index = 0usize;
            for pair in t.clone().pairs::<Value, Value>() {
                let (k, _) = pair?;
                match k {
                    Value::Number(n) if n.fract() == 0.0 && n > 0.0 => {
                        let i = n as usize;
                        if i > max_index { max_index = i; }
                    }
                    _ => { is_array = false; break; }
                }
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
