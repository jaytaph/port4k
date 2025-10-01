use crate::lua::LuaJob;
use crate::input::parser::{Verb, parse_command};
use crate::state::registry::Registry;
use crate::state::session::Session;
use anyhow::Result;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

mod balance;
mod bp;
mod debug_cmd;
mod fallback;
mod go;
mod login;
mod logout;
mod look;
mod playtest;
mod register;
mod script;
mod take;
mod who;

/// Command context passed to command handlers
pub struct CmdCtx {
    pub registry: Arc<Registry>,
    pub sess: Arc<RwLock<Session>>,
    pub lua_tx: mpsc::Sender<LuaJob>,
}

pub async fn process_command(
    raw: &str,
    registry: Arc<Registry>,
    sess: Arc<RwLock<Session>>,
    lua_tx: mpsc::Sender<LuaJob>,
) -> Result<String> {
    let intent = parse_command(raw);

    let ctx = Arc::new(CmdCtx { registry, sess, lua_tx });

    match intent.verb {
        Verb::Unknown => Ok("Unknown command. Try `help`.\r\n".to_string()),
        Verb::Close => Ok("Goodbye!\r\n".to_string()),
        Verb::Help => Ok(help_text()),
        Verb::Look => look::look(ctx.clone(), intent).await,
        Verb::Take => take::take(ctx.clone(), intent).await,
        Verb::Drop => Ok("Drop command not implemented yet.\r\n".to_string()),
        Verb::Open => Ok("Open command not implemented yet.\r\n".to_string()),
        Verb::Unlock => Ok("Unlock command not implemented yet.\r\n".to_string()),
        Verb::Lock => Ok("Lock command not implemented yet.\r\n".to_string()),
        Verb::Use => Ok("Use command not implemented yet.\r\n".to_string()),
        Verb::Put => Ok("Put command not implemented yet.\r\n".to_string()),
        Verb::Talk => Ok("Talk command not implemented yet.\r\n".to_string()),
        Verb::Go => go::go(ctx.clone(), intent).await,
        Verb::Inventory => Ok("Inventory command not implemented yet.\r\n".to_string()),
        Verb::Quit => Ok("Goodbye!\r\n".to_string()),
        Verb::Who => who::who(ctx.clone()).await,

        Verb::Logout => logout::logout(ctx.clone(), intent).await,
        Verb::Login => login::login(ctx.clone(), intent).await,
        Verb::Register => register::register(ctx.clone(), intent).await,
        // "help" => Ok(help_text()),
        // "quit" | "exit" => Ok("Goodbye!\r\n".to_string()),
        // "who" => balance::who(&ctx).await, // tiny helper in balance.rs (or move to its own file)
        // "register" => login::register(&ctx, it.collect()).await,
        // "login" => login::login(&ctx, it.collect()).await, // one-line login; telnet 2-step stays in connection.rs
        // "look" => look::look(&ctx).await,
        // "go" => go::go(&ctx, it.collect()).await,
        // "take" => take::take(&ctx, it.collect()).await,
        // "balance" => balance::balance(&ctx).await,
        //
        // // Namespaced commands
        // v if v.starts_with('@') => match &v[1..] {
        //     "bp" => bp::bp(&ctx, raw).await,
        //     "playtest" => playtest::playtest(&ctx, raw).await,
        //     "debug" => debug_cmd::debug(&ctx, raw).await,
        //     "script" => script::script(&ctx, raw).await,
        //     _ => Ok("Unknown @-command. Try `help`.\r\n".into()),
        // },
        //
        // // Fallback (e.g., playtest Lua on_command)
        // _ => fallback::fallback(&ctx, verb, it.map(|s| s.to_string()).collect()).await,
    }
}

pub fn help_text() -> String {
    r#"
Available commands\r\n
------------------\r\n
  help                         Show this help\r\n
  register <name> <password>   Create a new account\r\n
  login <name> <password>      Log in (WebSocket or one-line)\r\n
  login <name>                 (Telnet two-step is supported; enter just `login <name>`)\r\n
  who                          List online users\r\n
  look                         Look around your current room\r\n
  go <dir>                     Move (e.g., go north / go east)\r\n
  take coin [N]                Pick up up to N coins from the room\r\n
  balance                      Show how many coins you have\r\n
  quit                         Disconnect\r\n
\r\n
Special:\r\n
  @bp ...                      Manage blueprints and rooms\r\n
  @playtest [key|stop]         Enter/exit playtest mode\r\n
  @script ...                  Edit/publish Lua scripts\r\n
  @debug where                 Show debug info\r\n
"#
    .to_string()
}
