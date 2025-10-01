use crate::lua::LuaJob;
use crate::input::parser::{Verb, parse_command};
use crate::state::registry::Registry;
use crate::state::session::Session;
use anyhow::Result;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use crate::ansi;

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
        Verb::Unknown => Ok("Unknown command. Try `help`.\n".to_string()),
        Verb::Close => Ok("Goodbye!\n".to_string()),
        Verb::Help => Ok(help_text()),
        Verb::Look => look::look(ctx.clone(), intent).await,
        Verb::Take => take::take(ctx.clone(), intent).await,
        Verb::Drop => Ok("Drop command not implemented yet.\n".to_string()),
        Verb::Open => Ok("Open command not implemented yet.\n".to_string()),
        Verb::Unlock => Ok("Unlock command not implemented yet.\n".to_string()),
        Verb::Lock => Ok("Lock command not implemented yet.\n".to_string()),
        Verb::Use => Ok("Use command not implemented yet.\n".to_string()),
        Verb::Put => Ok("Put command not implemented yet.\n".to_string()),
        Verb::Talk => Ok("Talk command not implemented yet.\n".to_string()),
        Verb::Go => go::go(ctx.clone(), intent).await,
        Verb::Inventory => Ok("Inventory command not implemented yet.\n".to_string()),
        Verb::Quit => Ok("Goodbye!\n".to_string()),
        Verb::Who => who::who(ctx.clone()).await,

        Verb::Logout => logout::logout(ctx.clone(), intent).await,
        Verb::Login => login::login(ctx.clone(), intent).await,
        Verb::Register => register::register(ctx.clone(), intent).await,
        // "help" => Ok(help_text()),
        // "quit" | "exit" => Ok("Goodbye!\n".to_string()),
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
        //     _ => Ok("Unknown @-command. Try `help`.\n".into()),
        // },
        //
        // // Fallback (e.g., playtest Lua on_command)
        // _ => fallback::fallback(&ctx, verb, it.map(|s| s.to_string()).collect()).await,
    }
}

pub fn help_text() -> String {
    format!(
    r#"
{bold}{fg_cyan}Available commands{reset}
------------------
  {fg_yellow}help{reset}                         Show this help
  {fg_yellow}register <name> <password>{reset}   Create a new account
  {fg_yellow}login <name> <password>{reset}      Log in (WebSocket or one-line)
  {fg_yellow}login <name>{reset}                 (Telnet two-step is supported; enter just `login <name>`)
  {fg_yellow}who{reset}                          List online users
  {fg_yellow}look{reset}                         Look around your current room
  {fg_yellow}go <dir>{reset}                     Move (e.g., go north / go east)
  {fg_yellow}take coin [N]{reset}                Pick up up to N coins from the room
  {fg_yellow}balance{reset}                      Show how many coins you have
  {fg_yellow}quit{reset}                         Disconnect

{bold}{fg_cyan}Special:{reset}
  {fg_green}@bp ...{reset}                      Manage blueprints and rooms
  {fg_green}@playtest [key|stop]{reset}         Enter/exit playtest mode
  {fg_green}@script ...{reset}                  Edit/publish Lua scripts
  {fg_green}@debug where{reset}                 Show debug info
"#,
    bold = ansi::BOLD,
    fg_cyan = ansi::FG_CYAN,
    fg_yellow = ansi::FG_YELLOW,
    fg_green = ansi::FG_GREEN,
    reset = ansi::RESET,
    )
}
