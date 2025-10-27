use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use std::sync::Arc;

pub async fn logout(ctx: Arc<CmdCtx>, _intent: Intent) -> CommandResult {
    if !ctx.is_logged_in() {
        ctx.output.system("You must be logged in to log out.").await;
        return Ok(());
    }

    {
        let mut s = ctx.sess.write();
        s.state = ConnState::PreLogin;
        s.account = None;
        s.cursor = None;
    }

    ctx.output.system("You have been logged out.").await;
    Ok(())
}
