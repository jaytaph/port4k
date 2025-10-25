use crate::Registry;
use crate::error::{AppResult, DomainError};
use crate::input::parser::{Intent, Preposition, Quantifier};
use crate::models::account::Account;
use crate::models::room::{ResolvedExit, ResolvedObject, RoomView};
use crate::state::session::Cursor;
use mlua::{Function, Lua, Table};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::oneshot::Sender;
use crate::models::types::Direction;

pub const LUA_CMD_TIMEOUT: Duration = Duration::from_secs(5);

macro_rules! set_lua_table_readonly {
    ($table:expr, $lua:expr) => {{
        let mt = $lua.create_table()?;

        // Deny writes: t[k] = v
        mt.set(
            "__newindex",
            $lua.create_function(
                |_, (_t, _k, _v): (mlua::Table, mlua::Value, mlua::Value)| -> mlua::Result<()> {
                    Err(mlua::Error::RuntimeError("table is read-only".into()))
                },
            )?,
        )?;

        // Lock the metatable so scripts can't replace/remove it
        mt.set("__metatable", true)?;

        let _ = $table.set_metatable(Some(mt));
    }};
}


#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ScriptHook {
    /// First time a player enters the room
    #[serde(rename = "on_first_enter")]
    OnFirstEnter,
    /// Every time a player enters the room
    #[serde(rename = "on_enter")]
    OnEnter,
    /// When a player leaves the room
    #[serde(rename = "on_leave")]
    OnLeave,
    /// When a player issues a command in the room that is not handled elsewhere
    #[serde(rename = "on_command")]
    OnCommand,
}

impl ScriptHook {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScriptHook::OnFirstEnter => "on_first_enter",
            ScriptHook::OnEnter => "on_enter",
            ScriptHook::OnLeave => "on_leave",
            ScriptHook::OnCommand => "on_command",
        }
    }

    pub fn from_str(s: &str) -> AppResult<Self> {
        match s {
            "on_first_enter" => Ok(ScriptHook::OnFirstEnter),
            "on_enter" => Ok(ScriptHook::OnEnter),
            "on_leave" => Ok(ScriptHook::OnLeave),
            "on_command" => Ok(ScriptHook::OnCommand),
            _ => Err(DomainError::InvalidData(format!("unknown script hook: {}", s))),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LuaResult {
    Blocked,        // When checking is an exit is valid or not
    Success,        // Lua script executed successfully
    Failed(String),  // Lua script execution failed
}

impl From<DomainError> for LuaResult {
    fn from(error: DomainError) -> Self {
        LuaResult::Failed(error.to_string())
    }
}

impl From<mlua::Error> for LuaResult {
    fn from(error: mlua::Error) -> Self {
        LuaResult::Failed(format!("mlua: {}", error.to_string()))
    }
}
//
// impl FromLua for LuaResult {
//     fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
//         match value {
//             mlua::Value::Boolean(b) => {
//                 if b {
//                     Ok(LuaResult::Success)
//                 } else {
//                     Ok(LuaResult::Blocked)
//                 }
//             }
//             mlua::Value::String(s) => {
//                 let msg = s.to_str()?.to_string();
//                 Ok(LuaResult::Failed(msg))
//             }
//             _ => Err(mlua::Error::FromLuaConversionError {
//                 from: value.type_name(),
//                 to: "LuaResult",
//                 message: Some("expected boolean or string".into()),
//             }),
//         }
//     }
// }

// #[derive(Debug)]
// pub struct LuaResult {
//     /// Was the command successful (true) or failed (false)
//     pub ok: bool,
//     /// Output messages from the script
//     pub data: Vec<String>,
// }

pub enum LuaJob {
    /// Called when a player enters a room for the first time.
    OnFirstEnter {
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        reply: Sender<LuaResult>,
    },
    /// Called when a player enters a room.
    OnEnter {
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        reply: Sender<LuaResult>,
    },
    /// Called when a player leaves a room.
    OnLeave {
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        reply: Sender<LuaResult>,
    },
    /// Called when a player issues a command in a room
    OnCommand {
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        /// Intent of the command
        intent: Box<Intent>,
        reply: Sender<LuaResult>,
    },
    OnObject {
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        /// Intent of the command
        intent: Box<Intent>,
        /// Target object
        obj: Box<ResolvedObject>,
        reply: Sender<LuaResult>,
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
                    let ctx = LuaArgContext::new(cursor, account, registry.clone(), rt_handle.clone());
                    handle_room_script(&lua, &ctx, ScriptHook::OnEnter, reply);
                }
                LuaJob::OnFirstEnter { cursor, account, reply } => {
                    let ctx = LuaArgContext::new(cursor, account, registry.clone(), rt_handle.clone());
                    handle_room_script(&lua, &ctx, ScriptHook::OnFirstEnter, reply);
                }
                LuaJob::OnLeave { cursor, account, reply } => {
                    let ctx = LuaArgContext::new(cursor, account, registry.clone(), rt_handle.clone());
                    handle_room_script(&lua, &ctx, ScriptHook::OnLeave, reply);
                }
                LuaJob::OnObject { cursor, account, intent, obj, reply } => {
                    let ctx = LuaArgContext::new(cursor, account, registry.clone(), rt_handle.clone());
                    handle_object_script(&lua, &ctx, &intent, &obj, reply);
                }
                LuaJob::OnCommand { cursor, account, intent, reply } => {
                    let ctx = LuaArgContext::new(cursor, account, registry.clone(), rt_handle.clone());
                    handle_command_script(&lua, &ctx, &intent, reply);
                }
            };
        }
    });

    tx
}

struct LuaArgContext {
    output: Arc<Mutex<String>>,
    cursor: Box<Cursor>,
    rt_handle: Handle,
    registry: Arc<Registry>,
    account: Box<Account>,
}

impl LuaArgContext {
    fn new(cursor: Box<Cursor>, account: Account, registry: Arc<Registry>, rt_handle: Handle) -> Self {
        LuaArgContext {
            output: Arc::new(Mutex::new(String::new())),
            cursor: cursor.clone(),
            account: Box::new(account.clone()),
            registry,
            rt_handle,
        }
    }
}

impl Clone for LuaArgContext {
    fn clone(&self) -> Self {
        Self {
            output: self.output.clone(),
            cursor: self.cursor.clone(),
            registry: self.registry.clone(),
            account: self.account.clone(),
            rt_handle: self.rt_handle.clone(),
        }
    }
}

fn create_lua_env(lua: &Lua, arg_ctx: &LuaArgContext) -> mlua::Result<Table> {
    let env = make_env(&lua)?;

    let ctx = arg_ctx.clone();
    env.set("say", lua.create_function(move |_, (_self, msg): (Table, String)| -> mlua::Result<()> {
        ctx.output.lock().push_str(&msg);
        Ok(())
    })?)?;

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

    let ctx = arg_ctx.clone();
    env.set("set_exit_locked", lua.create_function({
        move |_, (_self, exit_dir, locked): (Table, String, Option<bool>)| {
            let ctx = ctx.clone();
            let zone_id = ctx.cursor.zone_ctx.zone.id;
            let room_id = ctx.cursor.room_view.room.id;
            let locked = locked.unwrap_or(true);

            let Some(dir) = Direction::parse(&exit_dir) else {
                return Err(mlua::Error::external(format!("invalid direction: {}", exit_dir)));
            };

            // Spawn async work in background
            ctx.rt_handle.spawn(async move {
                let _ = ctx.registry.services.room.set_exit_locked(
                    zone_id,
                    room_id,
                    ctx.account.id,
                    dir,
                    locked
                ).await;
            });

            Ok(())
        }
    })?)?;

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

fn install_host_api(
    lua: &Lua,
    env: &Table,
    cursor: &Cursor,
    out: Arc<Mutex<String>>,
) -> mlua::Result<()> {
    // ctx:send(text, newline = true)
    {
        let out = out.clone();
        let send = lua.create_function(move |_, (text, newline): (String, Option<bool>)| {
            let mut buf = out.lock();
            buf.push_str(&text);
            if newline.unwrap_or(true) {
                buf.push('\n');
            }
            Ok(())
        })?;
        env.set("send", send)?;
    }

    // ctx:broadcast_room(text, newline)
    {
        let out = out.clone();
        let f = lua.create_function(move |_, (text, newline): (String, Option<bool>)| {
            let mut buf = out.lock();
            buf.push_str(&text);
            if newline.unwrap_or(true) {
                buf.push('\n');
            }
            Ok(())
        })?;
        env.set("broadcast_room", f)?;
    }

    // // ctx:get_room() -> table
    // let cursor_clone = cursor.clone();
    // let get_room_fn = lua.create_function(move |lua, ()| -> mlua::Result<Table> {
    //     let t = lua.create_table()?;
    //
    //     let rt = create_lua_roomview_table(lua, &cursor_clone.room_view)?;
    //     t.set("room", rt)?;
    //
    //     // ----- exits (1-based array) -----
    //     let exits_tbl = lua.create_table()?;
    //     for (i, e) in cursor_clone.room_view.exits.iter().enumerate() {
    //         let et = create_lua_exit_table(lua, e)?;
    //         exits_tbl.raw_set(i + 1, et)?;
    //     }
    //     t.set("exits", exits_tbl)?;
    //
    //     // ----- objects (1-based array) -----
    //     let objs_tbl = lua.create_table()?;
    //     for (i, o) in cursor_clone.room_view.objects.iter().enumerate() {
    //         let ot = create_lua_object_table(lua, o)?;
    //         objs_tbl.raw_set(i + 1, ot)?;
    //     }
    //     t.set("objects", objs_tbl)?;
    //
    //     Ok(t)
    // })?;
    // env.set("get_room", get_room_fn)?;

    // ctx:get_object(key) -> table
    let cursor_clone = cursor.clone();
    let get_object_fn = lua.create_function(move |lua, (_self, obj_key): (Table, String)| -> mlua::Result<Option<Table>> {
        for o in cursor_clone.room_view.objects.iter() {
            if o.key == obj_key {
                let ot = create_lua_object_table(lua, o)?;
                return Ok(Some(ot));
            }
        }
        Ok(None)
    })?;
    env.set("get_object", get_object_fn)?;

    // // ctx:get_object_state(obj_key, key) -> table
    // let cursor_clone = cursor.clone();
    // let get_object_state_fn = lua.create_function(move |lua, (obj_key, state): (String, String)| -> mlua::Result<Option<String>> {
    //     match cursor_clone.room_view.object_by_key(&obj_key) {
    //         Some(obj) => {
    //             match state {
    //                 "hidden" => Ok(obj.flags.hidden),
    //                 "revealed" => Ok(obj.flags.revealed),
    //                 "locked" => Ok(obj.flags.locked),
    //                 "takeable" => Ok(obj.flags.takeable),
    //                 "stackable" => Ok(obj.flags.stackable),
    //                 _ => None,
    //             }
    //         }
    //         None => Ok(None),
    //     }
    // })?;
    // env.set("get_object_state", get_object_state_fn)?;

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

    set_lua_table_readonly!(et, lua);
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

    _ = set_lua_table_readonly!(t, lua);
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

    // ----- objects (1-based array) -----
    let objs_tbl = lua.create_table()?;
    for o in rv.objects.iter() {
        let ot = create_lua_object_table(lua, o)?;
        objs_tbl.raw_set(o.key.as_str(), ot)?;
    }
    rt.set("objects", objs_tbl)?;

    // ----- exits (1-based array) -----
    let exits_tbl = lua.create_table()?;
    for e in rv.exits.iter() {
        let et = create_lua_exit_table(lua, e)?;
        exits_tbl.raw_set(e.direction.as_str(), et)?;
    }
    rt.set("exits", exits_tbl)?;

    // Add kv
    let kv_tbl = lua.create_table()?;
    for (k, v) in rv.room_kv.iter() {
        kv_tbl.set(k.as_str(), v.as_str())?;
    }
    rt.set("state", kv_tbl)?;

    _ = set_lua_table_readonly!(rt, lua);
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

    _ = set_lua_table_readonly!(ot, lua);
    Ok(ot)
}

fn create_lua_intent_table(lua: &Lua, intent: &Intent) -> mlua::Result<Table> {
    let t = lua.create_table()?;

    t.set("verb", intent.verb.as_str())?;
    t.set("original", intent.original.as_str())?;
    t.set("raw_verb", intent.raw_verb.as_ref().map(String::as_str))?;

    let args_tbl = lua.create_table()?;
    for (i, a) in intent.args.iter().enumerate() {
        args_tbl.set(i + 1, a.as_str())?;
    }
    t.set("args", args_tbl)?;

    t.set("direction", intent.direction.as_ref().map(Direction::as_str))?;
    t.set("preposition", intent.preposition.as_ref().map(Preposition::as_str))?;
    t.set("quantifier", intent.quantifier.as_ref().map(Quantifier::as_str))?;

    _ = set_lua_table_readonly!(t, lua);
    Ok(t)
}

fn handle_room_script(lua: &Lua, ctx: &LuaArgContext, hook: ScriptHook, reply: Sender<LuaResult>) {
    let result = (|| -> AppResult<mlua::Value> {
        let binding = ctx.cursor.room_view.scripts.get(&hook);
        let src = binding.as_deref().map_or("", |s| s);

        if src.is_empty() {
            return Err(DomainError::Script("Empty script found".into()));
        }

        let env = create_lua_env(lua, ctx)?;
        install_host_api(lua, &env, &ctx.cursor, ctx.output.clone())?;

        let func: Function = lua
            .load(src)
            .set_name(format!(
                "{}:{}:{}",
                ctx.cursor.zone_ctx.blueprint.key,
                ctx.cursor.room_view.room.key,
                hook.as_str(),
            ))
            .eval()?;

        env.set("account", create_lua_account_table(lua, &ctx.account)?)?;
        env.set("room", create_lua_roomview_table(lua, &ctx.cursor.room_view)?)?;

        let result = func.call(env.clone())?;
        Ok(result)
    })();

    send_lua_result(reply, result)
}


fn handle_object_script(
    lua: &Lua,
    ctx: &LuaArgContext,
    intent: &Intent,
    obj: &ResolvedObject,
    reply: Sender<LuaResult>
) {
    let result = (|| -> AppResult<mlua::Value> {
        let Some(src) = &obj.use_lua else {
            return Err(DomainError::Script("No use script found on object".into()));
        };

        if src.is_empty() {
            return Err(DomainError::Script("Empty object use script found".into()));
        }

        let env = create_lua_env(lua, &ctx)?;
        install_host_api(lua, &env, &ctx.cursor, ctx.output.clone())?;

        let func: Function = lua
            .load(src)
            .set_name(format!("{}:on_use", obj.name))
            .eval()?;

        env.set("intent", create_lua_intent_table(lua, intent)?)?;
        env.set("account", create_lua_account_table(lua, &ctx.account)?)?;
        env.set("room", create_lua_roomview_table(lua, &ctx.cursor.room_view)?)?;
        env.set("obj", create_lua_object_table(lua, obj)?)?;

        let result = func.call(env.clone())?;
        Ok(result)
    })();

    send_lua_result(reply, result)
}

fn handle_command_script(
    lua: &Lua,
    ctx: &LuaArgContext,
    intent: &Intent,
    reply: Sender<LuaResult>
) {
    let result = (|| -> AppResult<mlua::Value> {
        let binding = ctx.cursor.room_view.scripts.get(&ScriptHook::OnCommand);
        let src = binding.as_deref().map_or("", |s| s);

        if src.is_empty() {
            return Err(DomainError::Script("Empty object use script found".into()));
        }

        let env = make_env(lua)?;
        install_host_api(lua, &env, &ctx.cursor, ctx.output.clone())?;

        env.set("intent", create_lua_intent_table(lua, intent)?)?;
        env.set("account", create_lua_account_table(lua, &ctx.account)?)?;
        env.set("room", create_lua_roomview_table(lua, &ctx.cursor.room_view)?)?;

        let func: Function = lua
            .load(src)
            .set_name(format!(
                "{}:{}:on_command",
                ctx.cursor.zone_ctx.blueprint.key,
                ctx.cursor.room_view.room.key
            ))
            .eval()?;

        let result = func.call(env.clone())?;
        Ok(result)
    })();

    send_lua_result(reply, result)
}


fn send_lua_result(reply: Sender<LuaResult>, result: AppResult<mlua::Value>) {
    let lua_val = match result {
        Ok(v) => v,
        Err(e) => {
            let lua_result = LuaResult::Failed(e.to_string());
            _ = reply.send(lua_result);
            return;
        }
    };


    let lua_result = match lua_val {
        mlua::Value::Table(tbl) => {
            let status: String = tbl.get("status").unwrap_or_else(|_| "failed".to_string());
            match status.as_str() {
                "blocked" => LuaResult::Blocked,
                "success" => LuaResult::Success,
                "failed" => {
                    let msg: String = tbl.get("message").unwrap_or_else(|_| "Unknown error".to_string());
                    LuaResult::Failed(msg)
                },
                _ => LuaResult::Failed(format!("Unknown status returned: {}", status)),
            }
        },
        mlua::Value::Nil => LuaResult::Success, // Default behavior
        _ => LuaResult::Failed("Invalid return type".into()),
    };

    _ = reply.send(lua_result);
}