use crate::input::parser::{Verb, parse_command};
use crate::state::session::Session;
use anyhow::Result;
use std::sync::{Arc, RwLock};
use crate::ansi;
use crate::commands::CommandResult::{Failure, Success};
use crate::net::AppState;

mod balance;
mod fallback;
mod go;
mod login;
mod logout;
mod look;
mod register;
mod take;
mod who;
mod blueprint;
mod debug_cmd;
mod playtest;
mod script;
mod admin;

/// Command context passed to command handlers
pub struct CmdCtx {
    pub state: Arc<AppState>,
    pub sess: Arc<RwLock<Session>>,
}

pub enum CommandResult {
    Success(String),
    Failure(String),
}

pub async fn process_command(
    raw: &str,
    state: Arc<AppState>,
    sess: Arc<RwLock<Session>>,
) -> Result<CommandResult> {
    let intent = parse_command(raw);

    let ctx = Arc::new(CmdCtx {
        state: state.clone(),
        sess: sess.clone()
    });

    match intent.verb {
        Verb::Close => Ok(Success("Goodbye!\n".to_string())),
        Verb::Help => Ok(Success(help_text())),
        Verb::Look => look::look(ctx.clone(), intent).await,
        Verb::Take => take::take(ctx.clone(), intent).await,
        Verb::Drop => Ok(Failure("Drop command not implemented yet.\n".to_string())),
        Verb::Open => Ok(Failure("Open command not implemented yet.\n".to_string())),
        Verb::Unlock => Ok(Failure("Unlock command not implemented yet.\n".to_string())),
        Verb::Lock => Ok(Failure("Lock command not implemented yet.\n".to_string())),
        Verb::Use => Ok(Failure("Use command not implemented yet.\n".to_string())),
        Verb::Put => Ok(Failure("Put command not implemented yet.\n".to_string())),
        Verb::Talk => Ok(Failure("Talk command not implemented yet.\n".to_string())),
        Verb::Go => go::go(ctx.clone(), intent).await,
        Verb::Inventory => Ok(Failure("Inventory command not implemented yet.\n".to_string())),
        Verb::Quit => Ok(Success("Goodbye!\n".to_string())),
        Verb::Who => who::who(ctx.clone()).await,

        Verb::Logout => logout::logout(ctx.clone(), intent).await,
        Verb::Login => login::login(ctx.clone(), intent).await,
        Verb::Register => register::register(ctx.clone(), intent).await,

        Verb::ScBlueprint => blueprint::blueprint(ctx.clone(), intent).await,
        Verb::ScPlaytest => playtest::playtest(ctx.clone(), intent).await,
        Verb::ScScript => script::script(ctx.clone(), intent).await,
        Verb::ScDebug => debug_cmd::debug_cmd(ctx.clone(), intent).await,

        Verb::Unknown => Ok(Failure("Unknown command. Try `help`.\n".to_string())),


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
