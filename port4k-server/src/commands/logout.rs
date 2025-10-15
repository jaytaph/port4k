use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;

pub async fn logout(ctx: Arc<CmdCtx>, _intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if !ctx.is_logged_in() {
        out.append("You must be logged in to log out.\n");
        out.failure();
        return Ok(out);
    }

    let mut s = ctx.sess.write();
    s.state = ConnState::PreLogin;
    s.account = None;
    s.cursor = None;

    out.append("You have been logged out.\n");
    out.success();
    Ok(out)
}
