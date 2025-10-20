use crate::Registry;
use crate::error::{AppResult, DomainError};
use crate::input::parser::Intent;
use crate::models::account::Account;
use crate::models::blueprint::Blueprint;
use crate::models::room::{ResolvedExit, ResolvedObject, RoomView};
use crate::state::session::Cursor;
use mlua::{Function, Lua, Table};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

#[derive(Debug)]
pub struct LuaResult {
    /// Was the command successful (true) or failed (false)
    pub ok: bool,
    /// Output messages from the script
    pub data: Vec<String>,
}

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
        reply: oneshot::Sender<Option<LuaResult>>,
    },
    OnObject {
        account: Account,
        cursor: Box<Cursor>,
        intent: Box<Intent>,
        obj: Box<ResolvedObject>,
        reply: oneshot::Sender<Option<LuaResult>>,
    },
}

/// Start a dedicated Lua worker thread with its own Lua state.
/// Pass the runtime `Handle` so the worker can run async DB calls with `handle.block_on(...)`.
pub fn start_lua_worker(rt_handle: Handle, registry: Arc<Registry>) -> mpsc::Sender<LuaJob> {
    let (tx, mut rx) = mpsc::channel::<LuaJob>(64);

    std::thread::spawn(move || {
        let lua = init_lua().expect("cannot init lua");
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
                        // Add ctx functions??
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

                LuaJob::OnObject { cursor, account, intent, obj, reply, } => {
                    debug!("LuaJob::OnObject: obj={:?}, intent={:?}", &obj.short, intent);
                    let lua_result = (|| -> AppResult<Option<LuaResult>> {
                        let Some(src) = &obj.use_lua else {
                            debug!("LuaJob::OnObject: no use_lua script");
                            return Ok(None);
                        };
                        if src.is_empty() {
                            debug!("LuaJob::OnObject: empty use_lua script");
                            return Ok(None);
                        }

                        // Output buffer
                        let output = Arc::new(Mutex::new(String::new()));

                        let arg_ctx = LuaArgContext {
                            output: output.clone(),
                            // cursor: cursor.clone(),
                            // rt_handle: rt_handle.clone(),
                            // registry: registry.clone(),
                            // obj: Some(obj.clone()),
                        };
                        let env = create_lua_env(&lua, arg_ctx)?;

                        install_host_api(&lua, &env, &rt_handle, &registry, &cursor, &account, output.clone())?;

                        debug!("LuaJob::OnObject: load eval");
                        let func: Function = lua.load(src)
                            .set_name(format!("{}:{}:{}:__entry", cursor.zone_ctx.blueprint.key, cursor.room_view.room.key, obj.name))
                            .eval()?;

                        // Build args table (1-based)
                        let t: Table = lua.create_table()?;
                        for (i, a) in intent.args.iter().enumerate() {
                            t.set(i + 1, a.as_str())?;
                        }

                        debug!("LuaJob::OnObject: calling returned function verb='{}'", intent.verb.as_str());

                        func.call::<()>((env.clone(), intent.verb.as_str(), t))?;

                        debug!("LuaJob::OnObject: function call complete");
                        let text = output.lock().clone();
                        Ok(Some(LuaResult {
                            ok: true,
                            data: text.lines().map(|s| s.to_string()).collect(),
                        }))
                    })();

                    debug!("LuaJob::OnObject: lua_result={:?}", &lua_result);
                    match lua_result {
                        Err(e) => {
                            debug!("LuaJob::OnObject: script error: {:?}", e);
                            eprintln!("Lua script error: {:?}", e);
                            let data = match e {
                                // DomainError::Lua(mlua::Error::RuntimeError(msg)) => msg.to_string(),
                                DomainError::Lua(e) => e.to_string(),
                                _ => "The room doesn't react (unknown script error)".to_string(),
                            };
                            let _ = reply.send(Some(LuaResult {
                                ok: false,
                                data: vec![data],
                            }));
                            continue;
                        }
                        Ok(None) => {
                            debug!("LuaJob::OnObject: no result from script");
                            let _ = reply.send(None);
                            continue;
                        }
                        Ok(Some(res)) => {
                            debug!("LuaJob::OnObject: sending result from script");
                            let _ = reply.send(Some(res));
                            continue;
                        }
                    }
                }

                LuaJob::OnCommand {
                    cursor,
                    account,
                    intent,
                    reply,
                } => {
                    let lua_result = (|| -> AppResult<Option<LuaResult>> {
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
                        let func: Function = lua.load(&src)
                            .set_name(format!("{}:{}:on_command", cursor.zone_ctx.blueprint.key, cursor.room_view.room.key))
                            .eval()?;

                        // Setup args table
                        let t: Table = lua.create_table()?;
                        for (i, a) in intent.args.iter().enumerate() {
                            t.set(i + 1, a.as_str())?;
                        }

                        debug!("LuaJob::OnObject: calling returned function verb='{}'", intent.verb.as_str());
                        // Pass ctx/env as first arg (Lua expects (ctx, verb, args))
                        func.call::<()>((env.clone(), intent.verb.as_str(), t))?;

                        debug!("LuaJob::OnObject: function call complete");
                        let text = out.lock().clone();
                        Ok(Some(LuaResult {
                            ok: true,
                            data: text.lines().map(|s| s.to_string()).collect(),
                        }))
                    })();

                    match lua_result {
                        Err(e) => {
                            eprintln!("Lua script error: {:?}", e);
                            let _ = reply.send(Some(LuaResult {
                                ok: false,
                                data: vec!["The room doesn't react (script error)".to_string()],
                            }));
                            continue;
                        }
                        Ok(None) => {
                            let _ = reply.send(None);
                            continue;
                        }
                        Ok(Some(res)) => {
                            let _ = reply.send(Some(res));
                            continue;
                        }
                    }
                }
            }
        }
    });

    tx
}

struct LuaArgContext {
    output: Arc<Mutex<String>>,
    // _cursor: Box<Cursor>,
    // _rt_handle: Handle,
    // _registry: Arc<Registry>,
    // _obj: Option<Box<ResolvedObject>>,
}

fn create_lua_env(lua: &Lua, arg_ctx: LuaArgContext) -> mlua::Result<Table> {
    let env = make_env(&lua)?;

    let output_clone = arg_ctx.output.clone();
    env.set("say", lua.create_function(move |_, (_self, msg): (Table, String)| -> mlua::Result<()> {
        output_clone.lock().push_str(&msg);
        Ok(())
    })?)?;

    // if let Some(obj) = arg_ctx.obj.as_ref() {
    // }

    // env.set("has_object", lua.create_function({
    // })?)?;
    //
    // env.set("reveal_object", lua.create_function({
    // })?)?;
    //
    // env.set("remove_object", lua.create_function({
    // })?)?;

    /*
    env.set("set_exit_locked_shared", lua.create_function({
        let cursor = cursor.clone();
        let handle = rt_handle.clone();
        let registry = registry.clone();
        move |_, (exit_key, locked): (String, bool)| -> mlua::Result<()> {
            let room_id = cursor.room_view.room.id;
            // let fut = registry.services.room.set_exit_locked(zone_id, room_id, &exit_key, locked);
            // handle.block_on(fut).map_err(mlua::Error::external)?;
            Ok(())
        }
    })?)?;
    */
    /*
    env.set("set_exit_locked", lua.create_function({
        let cursor = cursor.clone();
        let handle = rt_handle.clone();
        let registry = registry.clone();
        let account = account.clone();
        move |_, (exit_key, locked): (String, bool)| -> mlua::Result<()> {
            let room_id = cursor.room_view.room.id;
            // let fut = registry.services.room.set_exit_locked_player(zone_id, room_id, account.id, &exit_key, locked);
            // handle.block_on(fut).map_err(mlua::Error::external)?;
            Ok(())
        }
    })?)?;
    */

    Ok(env)
}

pub fn init_lua() -> anyhow::Result<Lua> {
    let lua = Lua::new();
    Ok(lua)
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
    _handle: &Handle,
    _registry: &Arc<Registry>,
    cursor: &Cursor,
    account: &Account,
    out: Arc<Mutex<String>>,
) -> mlua::Result<()> {
    // ctx:send(text)
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

    // ctx:broadcast_room(text)
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

    // ctx:get_account()
    {
        let t = create_lua_account_table(lua, account)?;
        env.set("get_account", t)?;
    }

    // ctx:get_room() -> table
    let cursor_clone = cursor.clone();
    let get_room_fn = lua.create_function(move |lua, ()| -> mlua::Result<Table> {
        let t = lua.create_table()?;

        let rt = create_lua_roomview_table(lua, &cursor_clone.room_view)?;
        t.set("room", rt)?;

        // ----- exits (1-based array) -----
        let exits_tbl = lua.create_table()?;
        for (i, e) in cursor_clone.room_view.exits.iter().enumerate() {
            let et = create_lua_exit_table(lua, e)?;
            exits_tbl.raw_set(i + 1, et)?;
        }
        t.set("exits", exits_tbl)?;

        // ----- objects (1-based array) -----
        let objs_tbl = lua.create_table()?;
        for (i, o) in cursor_clone.room_view.objects.iter().enumerate() {
            let ot = create_lua_object_table(lua, o)?;
            objs_tbl.raw_set(i + 1, ot)?;
        }
        t.set("objects", objs_tbl)?;

        Ok(t)
    })?;
    env.set("get_room", get_room_fn)?;

    // ctx:get_object(key) -> table
    let cursor_clone = cursor.clone();
    let get_object_fn = lua.create_function(move |lua, (obj_key,): (String,)| -> mlua::Result<Option<Table>> {
        for o in cursor_clone.room_view.objects.iter() {
            if o.key == obj_key {
                let ot = create_lua_object_table(lua, o)?;
                return Ok(Some(ot));
            }
        }
        Ok(None)
    })?;
    env.set("get_object", get_object_fn)?;

    // ctx:get_object_state(obj_key, key) -> table
    {}

    // ctx:set_object_state(obj_key, key, value)    (player state)
    {}

    // ctx:set_object_state_shared(obj_key, key, value) (zone state)
    {}

    // ctx:get_room_state(key) -> table
    {}

    // ctx:set_room_state(key, value)       (player state)
    {}

    // ctx:set_room_state_shared(key, value)  (zone state)
    {}

    // ctx:get_exit_state(dir, key) -> table
    {}

    // ctx:set_exit_state(dir, key, value)   (player state)
    {}

    // ctx:set_exit_state_shared(dir, key, value)  (zone state)
    {}

    Ok(())
}

fn create_lua_exit_table(lua: &Lua, exit: &ResolvedExit) -> mlua::Result<Table> {
    let et = lua.create_table()?;
    et.set("dir", exit.direction.to_string().as_str())?;
    et.set("from_room_key", exit.from_room_key.as_str())?;
    et.set("to_room_key", exit.to_room_key.as_str())?;
    et.set("locked", exit.flags.locked)?;
    et.set("exit", exit.flags.visible_when_locked)?;
    et.set("hidden", exit.flags.hidden)?;
    et.set("visible", exit.flags.is_visible())?;

    Ok(et)
}

fn create_lua_account_table(lua: &Lua, account: &Account) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("id", account.id.to_string())?;
    t.set("username", account.username.as_str())?;
    t.set("email", account.email.as_str())?;
    t.set("role", account.role.to_string().as_str())?;
    t.set("created_at", account.created_at.to_rfc3339())?;
    t.set("last_login", account.last_login.map(|dt| dt.to_rfc3339()).unwrap_or_default().as_str())?;

    Ok(t)
}

fn create_lua_roomview_table(lua: &Lua, rv: &RoomView) -> mlua::Result<Table> {
    let rt = lua.create_table()?;
    rt.set("id", rv.room.id.to_string())?;
    rt.set("key", rv.room.key.as_str())?;
    rt.set("title", rv.room.title.as_str())?;
    rt.set("description", rv.room.body.as_str())?;
    rt.set("short", rv.room.short.as_deref().unwrap_or(""))?;

    let hints = lua.create_table()?;
    for (i, h) in rv.room.hints.iter().enumerate() {
        let ht = lua.create_table()?;
        ht.set("text", h.text.as_str())?;
        ht.set("once", h.once.unwrap_or(false))?;
        ht.set("when", h.when.as_str())?;
        hints.raw_set(i + 1, ht)?;
    }
    rt.set("hints", hints)?;

    Ok(rt)
}

fn create_lua_object_table(lua: &Lua, obj: &ResolvedObject) -> mlua::Result<Table> {
    let ot = lua.create_table()?;
    ot.set("key", obj.key.as_str())?;
    ot.set("name", obj.name.as_str())?;
    ot.set("short", obj.short.as_str())?;
    ot.set("body", obj.description.as_str())?;
    ot.set("visible", obj.flags.is_visible())?;
    ot.set("takeable", obj.flags.takeable)?;
    ot.set("hidden", obj.flags.hidden)?;
    ot.set("revealed", obj.flags.revealed)?;
    ot.set("locked", obj.flags.locked)?;
    ot.set("stackable", obj.flags.stackable)?;

    Ok(ot)
}

// fn serde_json_to_lua(lua: &Lua, v: serde_json::Value) -> mlua::Result<Value> {
//     use serde_json::Value as J;
//     Ok(match v {
//         J::Null => Value::Nil,
//         J::Bool(b) => Value::Boolean(b),
//         J::Number(n) => {
//             if let Some(i) = n.as_i64() {
//                 Value::Integer(i as Integer)
//             } else if let Some(u) = n.as_u64() {
//                 // note: Lua integers are i64; clamp large u64s if needed
//                 Value::Integer(u as Integer)
//             } else {
//                 Value::Number(n.as_f64().unwrap_or(0.0))
//             }
//         }
//         J::String(s) => Value::String(lua.create_string(&s)?),
//         J::Array(arr) => {
//             let t = lua.create_table()?;
//             for (i, el) in arr.into_iter().enumerate() {
//                 t.set(i + 1, serde_json_to_lua(lua, el)?)?;
//             }
//             Value::Table(t)
//         }
//         J::Object(map) => {
//             let t = lua.create_table()?;
//             for (k, el) in map.into_iter() {
//                 t.set(k, serde_json_to_lua(lua, el)?)?;
//             }
//             Value::Table(t)
//         }
//     })
// }

// fn lua_to_serde_json(v: Value) -> mlua::Result<serde_json::Value> {
//     use serde_json::{Number, Value as J};
//     Ok(match v {
//         Value::Nil => J::Null,
//         Value::Boolean(b) => J::Bool(b),
//         Value::Integer(i) => J::Number(Number::from(i)),
//         Value::Number(n) => {
//             if n.is_finite() {
//                 J::Number(Number::from_f64(n).unwrap_or_else(|| Number::from(0)))
//             } else {
//                 J::Null
//             }
//         }
//         Value::String(s) => J::String(s.to_str()?.to_string()),
//         Value::Table(t) => {
//             // Prefer sequence if it looks like an array (1..N keys present)
//             if t.raw_len() > 0 && t.contains_key(1)? {
//                 let mut vec = Vec::with_capacity(t.raw_len());
//                 for val in t.sequence_values::<Value>() {
//                     vec.push(lua_to_serde_json(val?)?);
//                 }
//                 J::Array(vec)
//             } else {
//                 let mut map = serde_json::Map::new();
//                 for pair in t.pairs::<String, Value>() {
//                     let (k, vv) = pair?;
//                     map.insert(k, lua_to_serde_json(vv)?);
//                 }
//                 J::Object(map)
//             }
//         }
//         // functions, userdata, threads → represent as null for now
//         _ => J::Null,
//     })
// }
