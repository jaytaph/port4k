use crate::input::parser::{Verb, parse_command};
use crate::state::session::{Cursor, Session};
use std::sync::Arc;
use parking_lot::RwLock;
use crate::ansi;
use crate::models::account::Account;
use crate::models::types::AccountId;
use crate::error::CommandError;
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
mod admin;

pub type CommandResult<T> = Result<T, CommandError>;

/// Command context passed to command handlers
pub struct CmdCtx {
    pub state: Arc<AppState>,
    pub sess: Arc<RwLock<Session>>,
}

impl CmdCtx {
    #[inline]
    fn with_sess<T>(&self, f: impl FnOnce(&Session) -> T) -> CommandResult<T> {
        let s = self.sess.read();
        Ok(f(&s))
    }

    pub fn is_logged_in(&self) -> bool {
        self.sess.try_read().map_or(false, |s| s.account.is_some())
    }

    pub fn account_id(&self) -> CommandResult<AccountId> {
        self.with_sess(|s| s.account.as_ref().map(|a| a.id))
            .and_then(|opt| opt.ok_or(CommandError::NotLoggedIn))
    }

    pub fn account(&self) -> CommandResult<Account> {
        self.with_sess(|s| s.account.clone())
            .and_then(|opt| opt.ok_or(CommandError::NotLoggedIn))
    }

    pub fn has_cursor(&self) -> bool {
        self.sess.try_read().map_or(false, |s| s.cursor.is_some())
    }

    pub fn cursor(&self) -> CommandResult<Cursor> {
        self.with_sess(|s| s.cursor.clone())
            .and_then(|opt| opt.ok_or(CommandError::NoCursor))
    }
}

pub struct CommandOutput {
    pub message: String,
    pub is_error: bool,
}

#[macro_export]
macro_rules! success {
    ($msg:expr) => {
        CommandOutput { is_error: false, message: $msg.to_string() }
    };
}

#[macro_export]
macro_rules! failure {
    ($msg:expr) => {
        CommandOutput { is_error: true, message: $msg.to_string() }
    };
}

pub async fn process_command(
    raw: &str,
    state: Arc<AppState>,
    sess: Arc<RwLock<Session>>,
) -> CommandResult<CommandOutput> {
    let intent = parse_command(raw);

    let ctx = Arc::new(CmdCtx {
        state: state.clone(),
        sess: sess.clone()
    });

    match intent.verb {
        Verb::Close => Ok(success!("Goodbye!\n".to_string())),
        Verb::Help => Ok(success!(help_text())),
        Verb::Look => look::look(ctx.clone(), intent).await,
        Verb::Take => take::take(ctx.clone(), intent).await,
        Verb::Drop => Ok(failure!("Drop command not implemented yet.\n".to_string())),
        Verb::Open => Ok(failure!("Open command not implemented yet.\n".to_string())),
        Verb::Unlock => Ok(failure!("Unlock command not implemented yet.\n".to_string())),
        Verb::Lock => Ok(failure!("Lock command not implemented yet.\n".to_string())),
        Verb::Use => Ok(failure!("Use command not implemented yet.\n".to_string())),
        Verb::Put => Ok(failure!("Put command not implemented yet.\n".to_string())),
        Verb::Talk => Ok(failure!("Talk command not implemented yet.\n".to_string())),
        Verb::Go => go::go(ctx.clone(), intent).await,
        Verb::Inventory => Ok(failure!("Inventory command not implemented yet.\n".to_string())),
        Verb::Quit => Ok(success!("Goodbye!\n".to_string())),
        Verb::Who => who::who(ctx.clone()).await,

        Verb::Logout => logout::logout(ctx.clone(), intent).await,
        Verb::Login => login::login(ctx.clone(), intent).await,
        Verb::Register => register::register(ctx.clone(), intent).await,

        Verb::ScBlueprint => blueprint::blueprint(ctx.clone(), intent).await,
        Verb::ScPlaytest => playtest::playtest(ctx.clone(), intent).await,
        // Verb::ScScript => script::script(ctx.clone(), intent).await,
        Verb::ScDebug => debug_cmd::debug_cmd(ctx.clone(), intent).await,

        Verb::Unknown => Ok(failure!("Unknown command. Try `help`.\n".to_string())),

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
  {fg_green}@debug where{reset}                 Show debug info
"#,
    bold = ansi::BOLD,
    fg_cyan = ansi::FG_CYAN,
    fg_yellow = ansi::FG_YELLOW,
    fg_green = ansi::FG_GREEN,
    reset = ansi::RESET,
    )
}
