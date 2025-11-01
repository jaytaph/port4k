use crate::db::error::DbError;
use crate::error::{AppResult, DomainError};
use crate::input::parser::{Verb, parse_command};
use crate::input::shell::{handle_shell_cmd, parse_shell_cmd};
use crate::lua::LuaJob;
use crate::models::account::Account;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, RealmId, RoomId};
use crate::net::output::OutputHandle;
use crate::services::ServiceError;
use crate::state::session::{Cursor, Session};
use crate::{Registry, ansi};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;

mod blueprint;
mod debug_cmd;
mod examine;
mod fallback;
mod go;
mod inventory;
mod login;
mod logout;
mod look;
mod lua;
mod open;
mod register;
mod search;
mod take;
mod who;

pub type CommandResult = Result<(), CommandError>;

#[async_trait]
pub trait Command {
    async fn run(&self, ctx: Arc<CmdCtx>) -> CommandResult;
}

//noinspection RsExternalLinter
#[derive(Debug, Error)]
pub enum CommandError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),

    #[error("usage: {0}")]
    Usage(String),

    #[error("permission denied")]
    PermissionDenied,

    #[error("not logged in")]
    NotLoggedIn,

    #[error("cursor not found")]
    NoCursor,

    #[error(transparent)]
    Send(#[from] Box<SendError<LuaJob>>),

    #[error("custom error: {0}")]
    Custom(String),

    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Service(#[from] ServiceError),

    #[error(transparent)]
    Db(#[from] DbError),

    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
}

/// Command context passed to command handlers
pub struct CmdCtx {
    /// Output system
    pub output: OutputHandle,
    /// Global service registry
    pub registry: Arc<Registry>,
    /// Channel to send jobs to the Lua thread
    pub lua_tx: mpsc::Sender<LuaJob>,
    /// Player session
    pub sess: Arc<RwLock<Session>>,
}

impl CmdCtx {
    #[inline]
    fn with_sess<T>(&self, f: impl FnOnce(&Session) -> T) -> AppResult<T> {
        let s = self.sess.read();
        Ok(f(&s))
    }

    pub fn is_logged_in(&self) -> bool {
        self.sess.try_read().is_some_and(|s| s.get_account().is_some())
    }

    pub fn realm_id(&self) -> AppResult<RealmId> {
        self.with_sess(|s| s.get_cursor().as_ref().map(|c| c.realm.id))
            .and_then(|opt| opt.ok_or(DomainError::NotLoggedIn))
    }

    pub fn account_id(&self) -> AppResult<AccountId> {
        self.with_sess(|s| s.get_account().as_ref().map(|a| a.id))
            .and_then(|opt| opt.ok_or(DomainError::NotLoggedIn))
    }

    pub fn room_id(&self) -> AppResult<RoomId> {
        self.cursor().map(|c| c.room.blueprint.id)
    }

    pub fn account(&self) -> AppResult<Arc<Account>> {
        self.with_sess(|s| s.get_account())
            .and_then(|opt| opt.ok_or(DomainError::NotLoggedIn))
    }

    pub fn cursor(&self) -> AppResult<Cursor> {
        self.with_sess(|s| s.get_cursor())
            .and_then(|opt| opt.ok_or(DomainError::NoCurrentRoom))
    }

    pub fn has_cursor(&self) -> bool {
        self.sess.try_read().is_some_and(|s| s.get_cursor().is_some())
    }

    pub fn room_view(&self) -> AppResult<Arc<RoomView>> {
        Ok(self.cursor()?.room)
    }
}

pub async fn process_command(raw: &str, ctx: Arc<CmdCtx>) -> CommandResult {
    // See if we match a shell command, and handle it if so
    if let Some(shell) = parse_shell_cmd(raw) {
        handle_shell_cmd(shell, ctx.clone()).await?;
        return Ok(());
    }

    let intent = parse_command(raw);
    match intent.verb {
        Verb::Close => {
            ctx.output.system("Goodbye! Connection closed by user.").await;
            Ok(())
        }
        Verb::Help => {
            ctx.output.system(help_text()).await;
            Ok(())
        }
        Verb::Look => look::look(ctx.clone(), intent).await,
        Verb::Examine => examine::examine(ctx.clone(), intent).await,
        Verb::Search => search::search(ctx.clone(), intent).await,
        Verb::Take => take::take(ctx.clone(), intent).await,
        Verb::Drop => {
            ctx.output.system("Drop command not implemented yet.").await;
            Ok(())
        }
        Verb::Open => open::open(ctx.clone(), intent).await,
        Verb::Unlock => {
            ctx.output.system("Unlock command not implemented yet.").await;
            Ok(())
        }
        Verb::Lock => {
            ctx.output.system("Lock command not implemented yet.").await;
            Ok(())
        }
        Verb::Use => {
            ctx.output.system("Use command not implemented yet.").await;
            Ok(())
        }
        Verb::Put => {
            ctx.output.system("Put command not implemented yet.").await;
            Ok(())
        }
        Verb::Talk => {
            ctx.output.system("Talk command not implemented yet.").await;
            Ok(())
        }
        Verb::Go => go::go(ctx.clone(), intent).await,
        Verb::Inventory => inventory::inventory(ctx.clone(), intent).await,
        Verb::Quit => {
            ctx.output.system("Goodbye! Connection closed by user.").await;
            Ok(())
        }
        Verb::Who => who::who(ctx.clone()).await,
        Verb::Logout => logout::logout(ctx.clone(), intent).await,
        Verb::Login => login::login(ctx.clone(), intent).await,
        Verb::Register => register::register(ctx.clone(), intent).await,
        Verb::LuaRepl => lua::repl(ctx.clone()).await,
        // Verb::ScBlueprint => blueprint::blueprint(ctx.clone(), intent).await,
        // // Verb::ScScript => script::script(ctx.clone(), intent).await,
        // Verb::ScDebug => debug_cmd::debug_cmd(ctx.clone(), intent).await,
        // Fallback (e.g., playtest Lua on_command)
        Verb::Custom(_) => fallback::fallback(ctx.clone(), intent).await,
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
