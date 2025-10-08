use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use crate::{failure, success};

pub async fn logout(ctx: Arc<CmdCtx>, _intent: Intent) -> CommandResult<CommandOutput> {
    if !ctx.is_logged_in() {
        return Ok(failure!("You must be logged in to log out.\n"));
    }

    let mut s = ctx.sess.write();
    s.state = ConnState::PreLogin;
    s.account = None;
    s.cursor = None;

    Ok(success!("You have been logged out.\n"))
}
