use crate::input::parser::{Verb, parse_command};
use crate::state::session::{Cursor, Session};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use crate::{ansi, Registry};
use crate::db::error::DbError;
use crate::error::{AppResult, DomainError};
use crate::input::shell::{handle_shell_cmd, parse_shell_cmd};
use crate::models::account::Account;
use crate::models::types::AccountId;
use crate::lua::LuaJob;
use crate::models::room::RoomView;
use crate::models::zone::ZoneContext;
use crate::services::ServiceError;

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
mod examine;
mod search;

pub type CommandResult<T> = Result<T, CommandError>;

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
    Send(#[from] SendError<LuaJob>),

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
    /// Global service registry
    pub registry: Arc<Registry>,
    /// Channel to send jobs to the Lua thread
    pub lua_tx: mpsc::Sender<LuaJob>,
    /// Player session
    pub sess: Arc<RwLock<Session>>,
    // /// Current zone context
    // pub zone_ctx: Option<ZoneCtx>,
}

impl CmdCtx {
    #[inline]
    fn with_sess<T>(&self, f: impl FnOnce(&Session) -> T) -> AppResult<T> {
        let s = self.sess.read();
        Ok(f(&s))
    }

    pub fn is_logged_in(&self) -> bool {
        self.sess.try_read().map_or(false, |s| s.account.is_some())
    }

    pub fn account_id(&self) -> AppResult<AccountId> {
        self.with_sess(|s| s.account.as_ref().map(|a| a.id))
            .and_then(|opt| opt.ok_or(DomainError::NotLoggedIn))
    }

    pub fn account(&self) -> AppResult<Account> {
        self.with_sess(|s| s.account.clone())
            .and_then(|opt| opt.ok_or(DomainError::NotLoggedIn))
    }

    pub fn has_zone_ctx(&self) -> bool {
        self.sess.try_read().map_or(false, |s| s.zone_ctx.is_some())
    }

    pub fn zone_ctx(&self) -> AppResult<ZoneContext> {
        self.with_sess(|s| s.zone_ctx.clone())
            .and_then(|opt| opt.ok_or(DomainError::NotFound))
    }

    pub fn cursor(&self) -> AppResult<Cursor> {
        self.with_sess(|s| s.cursor.clone())
            .and_then(|opt| opt.ok_or(DomainError::NotFound))
    }

    pub fn has_cursor(&self) -> bool {
        self.sess.try_read().map_or(false, |s| s.cursor.is_some())
    }

    pub fn room_view(&self) -> AppResult<RoomView> {
        Ok(self.cursor()?.room_view)
    }
}


pub async fn process_command(
    raw: &str,
    ctx: Arc<CmdCtx>,
) -> CommandResult<CommandOutput> {

    // See if we match a shell command, and handle it if so
    if let Some(shell) = parse_shell_cmd(&raw) {
        let out = handle_shell_cmd(shell, ctx.clone()).await?;
        return Ok(out);
    }

    let mut out = CommandOutput::new();

    let intent = parse_command(raw);
    match intent.verb {
        Verb::Close => {
            out.append("Goodbye! Connection closed by user.\n");
            out.success();
            Ok(out)
        },
        Verb::Help => {
            out.append(help_text().as_str());
            out.success();
            Ok(out)
        },
        Verb::Look => look::look(ctx.clone(), intent).await,
        Verb::Examine => examine::examine(ctx.clone(), intent).await,
        Verb::Search => search::search(ctx.clone(), intent).await,
        Verb::Take => take::take(ctx.clone(), intent).await,
        Verb::Drop => {
            out.append("Drop command not implemented yet.\n");
            out.failure();
            Ok(out)
        },
        Verb::Open => {
            out.append("Open command not implemented yet.\n");
            out.failure();
            Ok(out)
        },
        Verb::Unlock => {
            out.append("Unlock command not implemented yet.\n");
            out.failure();
            Ok(out)
        },
        Verb::Lock => {
            out.append("Lock command not implemented yet.\n");
            out.failure();
            Ok(out)
        },
        Verb::Use => {
            out.append("Use command not implemented yet.\n");
            out.failure();
            Ok(out)
        },
        Verb::Put => {
            out.append("Put command not implemented yet.\n");
            out.failure();
            Ok(out)
        },
        Verb::Talk => {
            out.append("Talk command not implemented yet.\n");
            out.failure();
            Ok(out)
        },
        Verb::Go => go::go(ctx.clone(), intent).await,
        Verb::Inventory => {
            out.append("Inventory command not implemented yet.\n");
            out.failure();
            Ok(out)
        },
        Verb::Quit => {
            out.append("Goodbye! Connection closed by user.\n");
            out.success();
            Ok(out)
        },
        Verb::Who => who::who(ctx.clone()).await,
        Verb::Logout => logout::logout(ctx.clone(), intent).await,
        Verb::Login => login::login(ctx.clone(), intent).await,
        Verb::Register => register::register(ctx.clone(), intent).await,

        Verb::ScBlueprint => blueprint::blueprint(ctx.clone(), intent).await,
        Verb::ScPlaytest => playtest::playtest(ctx.clone(), intent).await,
        // Verb::ScScript => script::script(ctx.clone(), intent).await,
        Verb::ScDebug => debug_cmd::debug_cmd(ctx.clone(), intent).await,

        // Fallback (e.g., playtest Lua on_command)
        Verb::Unknown => fallback::fallback(ctx.clone(), intent).await,
        // _ => Ok(failure!("Unknown command. Try `help`.\n".to_string())),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CmdStatus {
    Success,
    Failure,
    Neutral,
}

pub struct CommandOutput {
    pub status: CmdStatus,
    pub lines: Vec<String>,
    // more fields we might want later
}

impl CommandOutput {
    pub fn new() -> Self {
        Self {
            status: CmdStatus::Neutral,
            lines: Vec::new(),
        }
    }

    pub fn append(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn success(&mut self) {
        self.status = CmdStatus::Success;
    }

    pub fn failure(&mut self) {
        self.status = CmdStatus::Failure;
    }

    pub fn failed(&self) -> bool {
        self.status == CmdStatus::Failure
    }

    pub fn succeeded(&self) -> bool {
        self.status == CmdStatus::Success
    }

    pub fn message(&self) -> String {
        self.lines.join("")
    }

    pub fn messages(&self) -> Vec<String> {
        self.lines.clone()
    }
}