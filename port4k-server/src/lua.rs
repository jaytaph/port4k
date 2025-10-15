use crate::Registry;
use crate::error::AppResult;
use crate::input::parser::Intent;
use crate::models::account::Account;
use crate::models::blueprint::Blueprint;
use crate::models::room::RoomView;
use crate::state::session::Cursor;
use mlua::{Function, Integer, Lua, Table, Value};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};

//noinspection RsExternalLinter
pub enum LuaJob {
    /// Called when a player enters a room.
    OnEnter {
        account: Account,
        cursor: Box<Cursor>,
        reply: oneshot::Sender<bool>,
    },
    /// Called when a player issues a command in a room
    OnCommand {
        account: Account,
        cursor: Box<Cursor>,
        intent: Box<Intent>,
        reply: oneshot::Sender<bool>,
    },
    // // Called when a player issues a command in playtest mode (no DB state, just ephemeral)
    // OnCommandPlaytest {
    //     db: Arc<Db>,
    //     bp: Blueprint,
    //     room: RoomView,
    //     account: Account,
    //     verb: String,
    //     args: Vec<String>,
    //     reply: oneshot::Sender<AppResult<Option<String>>>,
    // },
    // // Called when we enter a room in playtest mode (no DB state, just ephemeral)
    // OnEnterPlaytest {
    //     db: Arc<Db>,
    //     bp: Blueprint,
    //     room: RoomView,
    //     account: Account,
    //     reply: oneshot::Sender<AppResult<Option<String>>>,
    // },
}

/// Start a dedicated Lua worker thread with its own Lua state.
/// Pass the runtime `Handle` so the worker can run async DB calls with `handle.block_on(...)`.
pub fn start_lua_worker(rt_handle: Handle, registry: Arc<Registry>) -> mpsc::Sender<LuaJob> {
    let (tx, mut rx) = mpsc::channel::<LuaJob>(64);

    std::thread::spawn(move || {
        let lua = Lua::new();
        lua.sandbox(true).expect("cannot sandbox lua");

        while let Some(job) = rx.blocking_recv() {
            match job {
                LuaJob::OnEnter { cursor, account, reply } => {
                    let _ = (|| -> AppResult<Option<String>> {
                        let src = cursor
                            .room_view
                            .scripts
                            .on_enter_lua
                            .as_deref()
                            .unwrap_or("")
                            .to_owned();
                        if src.is_empty() {
                            return Ok(None);
                        }

                        let out = Arc::new(Mutex::new(String::new()));

                        let env = make_env(&lua)?;
                        install_host_api(&lua, &env, &rt_handle, &registry, &cursor, &account, out.clone())?;

                        let chunk = lua
                            .load(&src)
                            .set_name(format!(
                                "{}:{}:on_enter",
                                cursor.zone_ctx.blueprint.key, cursor.room_view.room.key
                            ))
                            .set_environment(env.clone());
                        chunk.exec()?;

                        if let Ok(f) = env.get::<Function>("on_enter") {
                            let ctx = make_enter_ctx(&lua, &cursor.zone_ctx.blueprint, &cursor.room_view, &account)?;
                            f.call::<()>(ctx)?;
                        }

                        let text = out.lock().clone();
                        Ok(Some(text))
                    })();

                    let _ = reply.send(true);
                }

                LuaJob::OnCommand {
                    cursor,
                    account,
                    intent,
                    reply,
                } => {
                    let _ = (|| -> AppResult<Option<String>> {
                        let src = cursor
                            .room_view
                            .scripts
                            .on_command_lua
                            .as_deref()
                            .unwrap_or("")
                            .to_owned();
                        if src.is_empty() {
                            return Ok(None);
                        }

                        // Output buffer
                        let out = Arc::new(Mutex::new(String::new()));

                        let env = make_env(&lua)?;
                        install_host_api(&lua, &env, &rt_handle, &registry, &cursor, &account, out.clone())?;

                        // ----- Load & run on_command(bp:room) -----
                        lua.load(&src)
                            .set_name(format!(
                                "{}:{}:on_command",
                                cursor.zone_ctx.blueprint.key, cursor.room_view.room.key
                            ))
                            .exec()?;

                        if let Ok(func) = lua.globals().get::<Function>("on_command") {
                            let t: Table = lua.create_table()?;
                            for (i, a) in intent.args.iter().enumerate() {
                                t.set(i + 1, a.as_str())?;
                            }

                            // Synchronous call (keeps worker future Send)
                            func.call::<()>((intent.verb.as_str(), t))?;

                            let text = out.lock().clone();
                            return Ok(Some(text));
                        }

                        Ok(None)
                    })();

                    let _ = reply.send(true);
                }
            }
        }
    });

    tx
}

fn make_env(lua: &Lua) -> mlua::Result<Table> {
    let env = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.set("__index", lua.globals())?;
    _ = env.set_metatable(Some(mt));
    Ok(env)
}

fn make_enter_ctx<'lua>(
    lua: &'lua Lua,
    bp: &Blueprint,
    room: &'lua RoomView,
    account: &'lua Account,
) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("blueprint_key", bp.key.as_str())?;
    t.set("room_key", room.room.key.as_str())?;
    t.set("room_title", room.room.title.as_str())?;
    t.set("account_id", account.id.to_string())?;
    t.set("username", account.username.as_str())?;
    Ok(t)
}

fn install_host_api(
    lua: &Lua,
    env: &Table,
    handle: &Handle,
    registry: &Arc<Registry>,
    cursor: &Cursor,
    account: &Account,
    out: Arc<Mutex<String>>,
) -> mlua::Result<()> {
    // send(text)
    {
        let out = out.clone();
        let send = lua.create_function(move |_, (text,): (String,)| {
            let mut buf = out.lock();
            buf.push_str(&text);
            buf.push('\n');
            Ok(())
        })?;
        env.set("send", send)?;
    }

    // broadcast_room(text)
    {
        let out = out.clone();
        let f = lua.create_function(move |_, (text,): (String,)| {
            let mut buf = out.lock();
            buf.push_str(&text);
            buf.push('\n');
            Ok(())
        })?;
        env.set("broadcast_room", f)?;
    }

    // get_state(key) -> any (JSON)
    {
        let room_id = cursor.room_view.room.id;
        let handle = handle.clone();
        let registry = registry.clone();
        let f = lua.create_function(move |lua_ctx, (key,): (String,)| {
            let v = handle
                .block_on(registry.services.room.room_kv_get(room_id, &key))
                .map_err(mlua::Error::external)?;

            serde_json_to_lua(lua_ctx, v)
        })?;
        env.set("get_state", f)?;
    }

    // set_state(key, value)
    {
        let room_id = cursor.room_view.room.id;
        let handle = handle.clone();
        let registry = registry.clone();
        let f = lua.create_function(move |_lua_ctx, (key, value): (String, Value)| {
            let v = lua_to_serde_json(value)?;
            handle
                .block_on(registry.services.room.room_kv_set(room_id, &key, &v))
                .map_err(mlua::Error::external)?;
            Ok(())
        })?;
        env.set("set_state", f)?;
    }

    // get_player(key) -> any (JSON)
    {
        let room_id = cursor.room_view.room.id;
        let account_id = account.id;
        let registry = registry.clone();
        let handle = handle.clone();
        let f = lua.create_function(move |lua_ctx, (key,): (String,)| {
            let v = handle
                .block_on(registry.services.room.player_kv_get(room_id, account_id, &key))
                .map_err(mlua::Error::external)?;

            match v {
                None => Ok(Value::Nil),
                Some(v) => serde_json_to_lua(lua_ctx, v),
            }
        })?;
        env.set("get_player", f)?;
    }

    // set_player(key, value)
    {
        let room_id = cursor.room_view.room.id;
        let account_id = account.id;
        let registry = registry.clone();
        let handle = handle.clone();
        let f = lua.create_function(move |_lua_ctx, (key, value): (String, Value)| {
            let v = lua_to_serde_json(value)?;
            handle
                .block_on(registry.services.room.player_kv_set(room_id, account_id, &key, &v))
                .map_err(mlua::Error::external)?;
            Ok(())
        })?;
        env.set("set_player", f)?;
    }

    Ok(())
}

fn serde_json_to_lua(lua: &Lua, v: serde_json::Value) -> mlua::Result<Value> {
    use serde_json::Value as J;
    Ok(match v {
        J::Null => Value::Nil,
        J::Bool(b) => Value::Boolean(b),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i as Integer)
            } else if let Some(u) = n.as_u64() {
                // note: Lua integers are i64; clamp large u64s if needed
                Value::Integer(u as Integer)
            } else {
                Value::Number(n.as_f64().unwrap_or(0.0))
            }
        }
        J::String(s) => Value::String(lua.create_string(&s)?),
        J::Array(arr) => {
            let t = lua.create_table()?;
            for (i, el) in arr.into_iter().enumerate() {
                t.set(i + 1, serde_json_to_lua(lua, el)?)?;
            }
            Value::Table(t)
        }
        J::Object(map) => {
            let t = lua.create_table()?;
            for (k, el) in map.into_iter() {
                t.set(k, serde_json_to_lua(lua, el)?)?;
            }
            Value::Table(t)
        }
    })
}

fn lua_to_serde_json(v: Value) -> mlua::Result<serde_json::Value> {
    use serde_json::{Number, Value as J};
    Ok(match v {
        Value::Nil => J::Null,
        Value::Boolean(b) => J::Bool(b),
        Value::Integer(i) => J::Number(Number::from(i)),
        Value::Number(n) => {
            if n.is_finite() {
                J::Number(Number::from_f64(n).unwrap_or_else(|| Number::from(0)))
            } else {
                J::Null
            }
        }
        Value::String(s) => J::String(s.to_str()?.to_string()),
        Value::Table(t) => {
            // Prefer sequence if it looks like an array (1..N keys present)
            if t.raw_len() > 0 && t.contains_key(1)? {
                let mut vec = Vec::with_capacity(t.raw_len());
                for val in t.sequence_values::<Value>() {
                    vec.push(lua_to_serde_json(val?)?);
                }
                J::Array(vec)
            } else {
                let mut map = serde_json::Map::new();
                for pair in t.pairs::<String, Value>() {
                    let (k, vv) = pair?;
                    map.insert(k, lua_to_serde_json(vv)?);
                }
                J::Object(map)
            }
        }
        // functions, userdata, threads → represent as null for now
        _ => J::Null,
    })
}
