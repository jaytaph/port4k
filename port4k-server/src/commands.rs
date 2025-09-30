use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use crate::lua::LuaJob;
use crate::state::registry::Registry;
use crate::state::session::Session;

mod balance;
mod bp;
mod debug_cmd;
mod fallback;
mod go;
mod login;
mod look;
mod playtest;
mod script;
mod take;

pub struct CmdCtx<'a> {
    pub registry: &'a Arc<Registry>,
    pub sess: &'a Arc<Mutex<Session>>,
    pub lua_tx: mpsc::Sender<LuaJob>,
}

pub async fn process_command(
    raw: &str,
    registry: &Arc<Registry>,
    sess: &Arc<Mutex<Session>>,
    lua_tx: mpsc::Sender<LuaJob>,
) -> Result<String> {
    if raw.trim().is_empty() {
        return Ok(String::new());
    }
    let mut it = raw.split_whitespace();
    let Some(verb) = it.next() else {
        return Ok(String::new());
    };

    // Commands starting with '@' → special namespace
    let ctx = CmdCtx {
        registry,
        sess,
        lua_tx,
    };

    match verb.to_ascii_lowercase().as_str() {
        "help" => Ok(help_text()),
        "quit" | "exit" => Ok("Goodbye!\r\n".to_string()),
        "who" => balance::who(&ctx).await, // tiny helper in balance.rs (or move to its own file)
        "register" => login::register(&ctx, it.collect()).await,
        "login" => login::login(&ctx, it.collect()).await, // one-line login; telnet 2-step stays in connection.rs
        "look" => look::look(&ctx).await,
        "go" => go::go(&ctx, it.collect()).await,
        "take" => take::take(&ctx, it.collect()).await,
        "balance" => balance::balance(&ctx).await,

        // Namespaced commands
        v if v.starts_with('@') => match &v[1..] {
            "bp" => bp::bp(&ctx, raw).await,
            "playtest" => playtest::playtest(&ctx, raw).await,
            "debug" => debug_cmd::debug(&ctx, raw).await,
            "script" => script::script(&ctx, raw).await,
            _ => Ok("Unknown @-command. Try `help`.\r\n".into()),
        },

        // Fallback (e.g., playtest Lua on_command)
        _ => fallback::fallback(&ctx, verb, it.map(|s| s.to_string()).collect()).await,
    }
}

pub fn help_text() -> String {
    r#"
Available commands
------------------
  help                         Show this help
  register <name> <password>   Create a new account
  login <name> <password>      Log in (WebSocket or one-line)
  login <name>                 (Telnet two-step is supported; enter just `login <name>`)
  who                          List online users
  look                         Look around your current room
  go <dir>                     Move (e.g., go north / go east)
  take coin [N]                Pick up up to N coins from the room
  balance                      Show how many coins you have
  quit                         Disconnect

Special:
  @bp ...                      Manage blueprints and rooms
  @playtest [key|stop]         Enter/exit playtest mode
  @script ...                  Edit/publish Lua scripts
  @debug where                 Show debug info
"#
    .to_string()
}
