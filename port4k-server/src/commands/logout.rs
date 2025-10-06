use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use crate::commands::CommandResult::{Failure, Success};
use crate::error::AppResult;

pub async fn logout(ctx: Arc<CmdCtx>, _intent: Intent) -> AppResult<CommandResult> {
    let mut s = ctx.sess.write();
    if s.state == ConnState::PreLogin {
        return Ok(Failure("You are already logged out.\n".into()));
    }

    s.state = ConnState::PreLogin;
    s.account = None;
    s.cursor = None;

    Ok(Success("You have been logged out.\n".into()))
}
