use crate::Registry;
use crate::error::{AppResult, DomainError};
use crate::input::parser::{Intent, Preposition, Quantifier};
use crate::models::account::Account;
use crate::models::room::{ResolvedExit, ResolvedObject, RoomView};
use crate::state::session::Cursor;
use mlua::{Function, Lua, Table};
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::oneshot::Sender;
use crate::models::types::Direction;
use crate::net::output::OutputHandle;

pub const LUA_CMD_TIMEOUT: Duration = Duration::from_secs(5);
const REPL_ENV_KEY : &str = "__repl_env";

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

        _ = $table.set_metatable(Some(mt));
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
    Success(mlua::Value),   // Lua script executed successfully
    Failed(String),         // Lua script execution failed
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

pub enum LuaJob {
    /// Called when a player enters a room for the first time.
    OnFirstEnter {
        /// Output handle for text,
        output_handle: OutputHandle,
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        /// Return channel
        reply: Sender<LuaResult>,
    },
    /// Called when a player enters a room.
    OnEnter {
        /// Output handle for text,
        output_handle: OutputHandle,
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        /// Return channel
        reply: Sender<LuaResult>,
    },
    /// Called when a player leaves a room.
    OnLeave {
        /// Output handle for text,
        output_handle: OutputHandle,
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        /// Return channel
        reply: Sender<LuaResult>,
    },
    /// Called when a player issues a command in a room
    OnCommand {
        /// Output handle for text,
        output_handle: OutputHandle,
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        /// Intent of the command
        intent: Box<Intent>,
        /// Return channel
        reply: Sender<LuaResult>,
    },
    OnObject {
        /// Output handle for text,
        output_handle: OutputHandle,
        /// Account of the user
        account: Account,
        /// Cursor of the user
        cursor: Box<Cursor>,
        /// Intent of the command
        intent: Box<Intent>,
        /// Target object
        obj: Box<ResolvedObject>,
        /// Return channel
        reply: Sender<LuaResult>,
    },

    ReplEval {
        /// Output handle for text,
        output_handle: OutputHandle,
        /// Cursor of the user
        cursor: Box<Cursor>,
        /// Account of the user
        account: Account,
        /// Given string to eval()
        code: String,
        /// Return channel
        reply: Sender<LuaResult>,
    }
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
                LuaJob::OnEnter { output_handle, cursor, account, reply } => {
                    let ctx = LuaArgContext::new(output_handle.clone(), Some(*cursor), Some(account), registry.clone(), rt_handle.clone());
                    handle_room_script(&lua, &ctx, ScriptHook::OnEnter, reply);
                }
                LuaJob::OnFirstEnter { output_handle, cursor, account, reply } => {
                    let ctx = LuaArgContext::new(output_handle.clone(), Some(*cursor), Some(account), registry.clone(), rt_handle.clone());
                    handle_room_script(&lua, &ctx, ScriptHook::OnFirstEnter, reply);
                }
                LuaJob::OnLeave { output_handle, cursor, account, reply } => {
                    let ctx = LuaArgContext::new(output_handle.clone(), Some(*cursor), Some(account), registry.clone(), rt_handle.clone());
                    handle_room_script(&lua, &ctx, ScriptHook::OnLeave, reply);
                }
                LuaJob::OnObject { output_handle, cursor, account, intent, obj, reply } => {
                    let ctx = LuaArgContext::new(output_handle.clone(), Some(*cursor), Some(account), registry.clone(), rt_handle.clone());
                    handle_object_script(&lua, &ctx, &intent, &obj, reply);
                }
                LuaJob::OnCommand { output_handle, cursor, account, intent, reply } => {
                    let ctx = LuaArgContext::new(output_handle.clone(), Some(*cursor), Some(account), registry.clone(), rt_handle.clone());
                    handle_command_script(&lua, &ctx, &intent, reply);
                }
                LuaJob::ReplEval { output_handle, cursor, account, code, reply } => {
                    let ctx = LuaArgContext::new(output_handle.clone(), Some(*cursor), Some(account), registry.clone(), rt_handle.clone());
                    _ = handle_repl_eval(&lua, &ctx, &code, reply);
                }
            };
        }
    });

    tx
}

struct LuaArgContext {
    /// Output handle
    output_handle: OutputHandle,
    /// Tokio Runtime handle
    rt_handle: Handle,
    /// Registry / service container
    registry: Arc<Registry>,
    /// Optional cursor (if in a room)
    cursor: Option<Box<Cursor>>,
    /// Optional account (if logged in)
    account: Option<Box<Account>>,
}

impl LuaArgContext {
    fn new(output_handle: OutputHandle, cursor: Option<Cursor>, account: Option<Account>, registry: Arc<Registry>, rt_handle: Handle) -> Self {
        LuaArgContext {
            output_handle,
            registry,
            rt_handle,
            cursor: cursor.map(|c| Box::new(c)),
            account: account.map(|a| Box::new(a)),
        }
    }
}

impl Clone for LuaArgContext {
    fn clone(&self) -> Self {
        Self {
            output_handle: self.output_handle.clone(),
            cursor: self.cursor.clone(),
            registry: self.registry.clone(),
            account: self.account.clone(),
            rt_handle: self.rt_handle.clone(),
        }
    }
}

fn create_lua_env(lua: &Lua, arg_ctx: &LuaArgContext) -> mlua::Result<Table> {
    let env = lua.create_table()?;

    let mt = lua.create_table()?;
    mt.set("__index", lua.globals())?;
    _ = env.set_metatable(Some(mt));

    env.set("port4k", create_port4k_function_table(lua, arg_ctx)?)?;

    // if let Some(account) = arg_ctx.account.as_ref() {
    //     env.set("account", create_lua_account_table(lua, &account)?)?;
    // }
    // if let Some(cursor) = arg_ctx.cursor.as_ref() {
    //     env.set("room", create_lua_roomview_table(lua, &cursor.room_view)?)?;
    // }

    env.set("_ENV", env.clone())?;

    Ok(env)
}

// fn install_game_api(lua: &Lua, env: &Table, arg_ctx: &LuaArgContext) -> mlua::Result<()> {
//     let ctx = arg_ctx.clone();
//     env.set("say", lua.create_function(move |_, msg: String| -> mlua::Result<()> {
//         let ctx = ctx.clone();
//         ctx.rt_handle.spawn(async move {
//             ctx.output_handle.line(msg).await;
//         });
//         Ok(())
//     })?)?;
//
//
//     let ctx = arg_ctx.clone();
//     env.set("say", lua.create_function(move |_, msg: String| -> mlua::Result<()> {
//         let ctx = ctx.clone();
//         ctx.rt_handle.spawn(async move {
//             ctx.output_handle.line(msg).await;
//         });
//         Ok(())
//     })?)?;
//
//
//     // env.set("has_object", lua.create_function({
//     // })?)?;
//     //
//     // env.set("reveal_object", lua.create_function({
//     // })?)?;
//     //
//     // env.set("remove_object", lua.create_function({
//     // })?)?;
//
//     /*
//     env.set("set_exit_locked_shared", lua.create_function({
//         let cursor = cursor.clone();
//         let handle = rt_handle.clone();
//         let registry = registry.clone();
//         move |_, (exit_key, locked): (String, bool)| -> mlua::Result<()> {
//             let room_id = cursor.room_view.room.id;
//             // let fut = registry.services.room.set_exit_locked(zone_id, room_id, &exit_key, locked);
//             // handle.block_on(fut).map_err(mlua::Error::external)?;
//             Ok(())
//         }
//     })?)?;
//     */
//
//     if let (Some(cursor), Some(account)) = (arg_ctx.cursor.as_ref(), arg_ctx.account.as_ref()) {
//         let cursor = cursor.clone();
//         let account = account.clone();
//         let ctx = arg_ctx.clone();
//
//         env.set("set_exit_locked", lua.create_function(move |_, (exit_dir, locked): (String, Option<bool>)| {
//             let zone_id = cursor.zone_ctx.zone.id;
//             let room_id = cursor.room_view.room.id;
//             let locked = locked.unwrap_or(true);
//
//             let Some(dir) = Direction::parse(&exit_dir) else {
//                 return Err(mlua::Error::external(format!("invalid direction: {}", exit_dir)));
//             };
//
//             let account = account.clone();
//             let ctx = ctx.clone();
//
//             // Spawn async work in background
//             ctx.rt_handle.spawn(async move {
//                 let _ = ctx.registry.services.room.set_exit_locked(
//                     zone_id,
//                     room_id,
//                     account.id,
//                     dir,
//                     locked
//                 ).await;
//             });
//
//             Ok(())
//         })?)?;
//     }
//
//     Ok(())
// }

pub fn init_lua() -> anyhow::Result<Lua> {
    let lua = Lua::new();
    Ok(lua)
}


fn create_port4k_function_table(
    lua: &Lua,
    arg_ctx: &LuaArgContext,
) -> mlua::Result<mlua::Table> {
    let port4k = lua.create_table()?;

    // port4k.say(text)
    let ctx = arg_ctx.clone();
    port4k.set("say", lua.create_function(move |_, msg: String| -> mlua::Result<()> {
        let ctx = ctx.clone();
        ctx.rt_handle.spawn(async move {
            ctx.output_handle.line(msg).await;
        });
        Ok(())
    })?)?;

    // port4k.broadcast(text)
    let ctx = arg_ctx.clone();
    port4k.set("broadcast", lua.create_function(move |_, msg: String| -> mlua::Result<()> {
        let ctx = ctx.clone();
        ctx.rt_handle.spawn(async move {
            ctx.output_handle.line(format!("BROADCAST: {}", msg)).await;
        });
        Ok(())
    })?)?;

    // port4k.set_exit_locked(exit: str, locked: bool)
    port4k.set("set_exit_locked", lua.create_function(move |_, (dir, locked): (String, bool)| -> mlua::Result<()> {
        let _ = dir;
        let _ = locked;
        Ok(())
    })?)?;

    // port4k.set_exit_locked(exit: str, locked: bool)
    port4k.set("is_exit_locked", lua.create_function(move |_, dir: String| -> mlua::Result<mlua::Value> {
        let _ = dir;
        Ok(mlua::Value::Boolean(true))
    })?)?;

    // port4k.player_has_item(item_key: str) -> bool
    port4k.set("is_exit_locked", lua.create_function(move |_, obj_key: String| -> mlua::Result<mlua::Value> {
        let _ = obj_key;
        Ok(mlua::Value::Boolean(true))
    })?)?;

    // port4k.set_object_state(key: str, value: str) -> bool
    port4k.set("set_object_state", lua.create_function(move |_, (obj_key, obj_value): (String, String)| -> mlua::Result<mlua::Value> {
        let _ = obj_key;
        let _ = obj_value;
        Ok(mlua::Value::Boolean(true))
    })?)?;

    // port4k.consume_item(key: str) -> bool
    port4k.set("consume_item", lua.create_function(move |_, obj_key: String| -> mlua::Result<mlua::Value> {
        let _ = obj_key;
        Ok(mlua::Value::Boolean(true))
    })?)?;

    // port4k.hint_trigger(hint_type: str) -> bool
    port4k.set("hint_trigger", lua.create_function(move |_, hint_type: String| -> mlua::Result<mlua::Value> {
        let _ = hint_type;
        Ok(mlua::Value::Boolean(true))
    })?)?;

    // port4k.hint_consider(hint_type: str) -> bool
    port4k.set("hint_consider", lua.create_function(move |_, hint_type: String| -> mlua::Result<mlua::Value> {
        let _ = hint_type;
        Ok(mlua::Value::Boolean(true))
    })?)?;

    // port4k.matches_noun(args: str, list: list) -> bool
    port4k.set("matches_noun", lua.create_function(move |_, (args, list): (String, String)| -> mlua::Result<mlua::Value> {
        let _ = args;
        let _ = list;
        Ok(mlua::Value::Boolean(true))
    })?)?;

    // port4k.current_room() -> room
    let arg_ctx = arg_ctx.clone();
    port4k.set("current_room", lua.create_function(move |lua, ()| -> mlua::Result<mlua::Value> {
        let t = create_lua_roomview_table(lua, &arg_ctx.cursor.as_ref().unwrap().room_view).map(mlua::Value::Table)?;
        Ok(t)
    })?)?;


    // // ctx:get_object(key) -> table
    // if let Some(cursor) = arg_ctx.cursor.as_ref() {
    //     let cursor = cursor.clone();
    //     env.set("get_object", lua.create_function(move |lua, obj_key: String| -> mlua::Result<Option<Table>> {
    //         for o in cursor.room_view.objects.iter() {
    //             if o.key == obj_key {
    //                 let ot = create_lua_object_table(lua, o)?;
    //                 return Ok(Some(ot));
    //             }
    //         }
    //         Ok(None)
    //     })?)?;
    // }

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


    Ok(port4k)
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
    t.set("last_login", account.last_login.map(|dt| dt.to_rfc3339()).as_deref().unwrap_or(""))?;

    _ = set_lua_table_readonly!(t, lua);
    Ok(t)
}

fn create_lua_roomview_table(lua: &Lua, rv: &RoomView) -> mlua::Result<Table> {
    let rt = lua.create_table()?;
    rt.set("id", rv.blueprint.id.to_string())?;
    rt.set("key", rv.blueprint.key.as_str())?;
    rt.set("title", rv.blueprint.title.as_str())?;
    rt.set("description", rv.blueprint.body.as_str())?;
    rt.set("short", rv.blueprint.short.as_deref().unwrap_or(""))?;

    let hints = lua.create_table()?;
    for (i, h) in rv.blueprint.hints.iter().enumerate() {
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

    // Add kv
    let kv_tbl = lua.create_table()?;
    for (k, v) in obj.kv.iter() {
        kv_tbl.set(k.as_str(), v.as_str())?;
    }
    ot.set("state", kv_tbl)?;

    _ = set_lua_table_readonly!(ot, lua);
    Ok(ot)
}

fn create_lua_intent_table(lua: &Lua, intent: &Intent) -> mlua::Result<Table> {
    let t = lua.create_table()?;

    t.set("verb", intent.verb.as_str())?;
    t.set("original", intent.original.as_str())?;
    t.set("raw_verb", intent.raw_verb.as_deref())?;

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
    let Some(cursor) = ctx.cursor.as_ref() else {
        let lua_result = LuaResult::Failed("No cursor available for room script".into());
        _ = reply.send(lua_result);
        return;
    };

    let result = (|| -> AppResult<mlua::Value> {
        let binding = cursor.room_view.scripts.get(&hook);
        let src = binding.as_deref().map_or("", |s| s);

        if src.is_empty() {
            return Err(DomainError::Script("Empty script found".into()));
        }

        let env = create_lua_env(lua, ctx)?;

        let args = lua.create_table()?;
        args.set("account", create_lua_account_table(lua, ctx.account.as_ref().unwrap())?)?;
        args.set("room", create_lua_roomview_table(lua, &cursor.room_view)?)?;

        let func: Function = lua
            .load(src)
            .set_name(format!(
                "{}:{}",
                cursor.room_view.blueprint.key,
                hook.as_str(),
            ))
            .set_environment(env)
            .eval()?;

        let result = func.call(args)?;
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
        let Some(src) = &obj.on_use else {
            return Err(DomainError::Script("No use script found on object".into()));
        };

        if src.is_empty() {
            return Err(DomainError::Script("Empty object use script found".into()));
        }

        let env = create_lua_env(lua, &ctx)?;

        let args = lua.create_table()?;
        args.set("account", create_lua_account_table(lua, ctx.account.as_ref().unwrap())?)?;
        args.set("intent", create_lua_intent_table(lua, intent)?)?;
        args.set("object", create_lua_object_table(lua, obj)?)?;

        let func: Function = lua
            .load(src)
            .set_name(format!("{}:on_use", obj.name))
            .set_environment(env)
            .eval()?;

        let result = func.call(args)?;
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
    let Some(cursor) = ctx.cursor.as_ref() else {
        let lua_result = LuaResult::Failed("No cursor and account available for room script".into());
        _ = reply.send(lua_result);
        return;
    };

    let result = (|| -> AppResult<mlua::Value> {
        let binding = cursor.room_view.scripts.get(&ScriptHook::OnCommand);
        let src = binding.as_deref().map_or("", |s| s);

        if src.is_empty() {
            return Err(DomainError::Script("Empty object use script found".into()));
        }

        let env = create_lua_env(lua, ctx)?;

        let args = lua.create_table()?;
        args.set("intent", create_lua_intent_table(lua, intent)?)?;
        args.set("room", create_lua_roomview_table(lua, &cursor.room_view)?)?;

        let func: Function = lua
            .load(src)
            .set_name(format!(
                "{}:on_command",
                cursor.room_view.blueprint.key
            ))
            .set_environment(env)
            .eval()?;

        let result = func.call(args)?;
        Ok(result)
    })();

    send_lua_result(reply, result)
}

fn handle_repl_eval(lua: &Lua, ctx: &LuaArgContext, code: &str, reply: Sender<LuaResult>) -> AppResult<()> {
    let ctx_table: Table = lua.named_registry_value(REPL_ENV_KEY)
        .or_else(|_| {
            // First time: create and store it
            let env = create_lua_env(lua, ctx)?;
            lua.set_named_registry_value(REPL_ENV_KEY, env.clone())?;
            Ok::<_, mlua::Error>(env)
        })?;

    let result = (|| -> Result<mlua::Value, mlua::Error> {
        match lua.load(format!("return {}", code))
            .set_environment(ctx_table.clone())
            .eval()
        {
            Ok(val) => Ok(val),
            Err(_) => {
                lua.load(code)
                    .set_environment(ctx_table.clone())
                    .eval()
            },
        }
    })();

    match result {
        Ok(value) => {
            _ = reply.send(LuaResult::Success(value));
            Ok(())
        },
        Err(err) => {
            _ = reply.send(LuaResult::Failed(format!("Lua error: {}", err.to_string())));
            Err(DomainError::ScriptLua(err))
        },
    }
}

fn send_lua_result(reply: Sender<LuaResult>, result: AppResult<mlua::Value>) {
    let lua_result = match result {
        // There was a value returned from lua (even if it's nil)
        Ok(v) => LuaResult::Success(v),
        // Error while excuting lua
        Err(e) => LuaResult::Failed(e.to_string()),
    };

    _ = reply.send(lua_result);
}
