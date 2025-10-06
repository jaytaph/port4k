use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::state::session::{ConnState, Cursor};
use anyhow::Result;
use crate::commands::CommandResult::{Failure, Success};
use crate::input::parser::Intent;

const USAGE: &str = "Usage:\n  playtest                # exit playtest\n  playtest <bp>           # enter playtest for blueprint <bp>\n";

enum Next {
    /// Contains the cursor to return to live mode
    ExitToLive(Cursor),
    /// Not currently in playtest mode
    NotInPlaytest,
    /// Not logged in
    NotLoggedIn,
    /// Some internal error
    Error
}


pub async fn playtest(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() == 1 {
        // No argument, so exit playtest
        return exit_playtest(ctx.clone()).await;
    }

    if intent.args.len() == 2 {
        // One argument, so enter playtest for the given blueprint
        return enter_playtest(ctx.clone(), intent.args[0].as_str()).await;
    }

    Ok(Failure(USAGE.into()))
}

pub fn check_playtest(ctx: Arc<CmdCtx>) -> Next {
    let Some(mut s) = ctx.sess.read().map_err(|_| anyhow::anyhow!("Could not acquire write lock")) else {
        return Next::Error;
    };

    if s.state != ConnState::LoggedIn {
        return Next::NotLoggedIn;
    }

    if s.prev_cursors.is_empty() {
        return Next::NotInPlaytest;
    }

    let c = s.prev_cursors.first().unwrap().clone();
    Next::ExitToLive(c)
}

pub async fn exit_playtest(ctx: Arc<CmdCtx>) -> Result<CommandResult> {
    match check_playtest(ctx.clone()) {
        Next::Error => Ok(Failure("Internal error.\n".into())),
        Next::NotLoggedIn => Ok(Failure("Login required.\n".into())),
        Next::NotInPlaytest => Ok(Failure("[playtest] you are not in playtest.\n".into())),
        Next::ExitToLive(c) => {
            let Some(mut s) = ctx.sess.write().map_err(|_| anyhow::anyhow!("Could not acquire write lock"))?;
            s.prev_cursors.pop();
            s.cursor = c;

            Ok(Success(format!("[playtest] exited.\n")))
        }
    }
}

pub async fn enter_playtest(ctx: Arc<CmdCtx>, bp_key: &str) -> Result<CommandResult> {
    let new_c = Cursor{};

    match check_playtest(ctx.clone()) {
        Next::Error => Ok(Failure("Internal error.\n".into())),
        Next::NotLoggedIn => Ok(Failure("Login required.\n".into())),
        Next::ExitToLive(_) => {
            let Some(mut s) = ctx.sess.write().map_err(|_| anyhow::anyhow!("Could not acquire write lock"))?;
            s.prev_cursors.push(s.cursor);
            s.cursor = new_c;

            Ok(Success(format!("[playtest] entered recursive blueprint '{bp_key}'.\n")))
        },
        Next::NotInPlaytest => {
            let Some(mut s) = ctx.sess.write().map_err(|_| anyhow::anyhow!("Could not acquire write lock"))?;
            s.prev_cursors.push(s.cursor);
            s.cursor = new_c;

            Ok(Success(format!("[playtest] entered blueprint '{bp_key}'.\n")))
        }
    }
}
