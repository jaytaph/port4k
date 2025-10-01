use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use anyhow::Result;
use crate::commands::CommandResult::{Failure, Success};

pub async fn logout(ctx: Arc<CmdCtx>, _intent: Intent) -> Result<CommandResult> {
    let mut s = ctx.sess.write().unwrap();
    if s.state == ConnState::PreLogin {
        return Ok(Failure("You are already logged out.\n".into()));
    }

    s.state = ConnState::PreLogin;
    s.name = None;
    s.world = None;

    Ok(Success("You have been logged out.\n".into()))
}
