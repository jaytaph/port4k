use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;

pub async fn logout(ctx: Arc<CmdCtx>, _intent: Intent) -> CommandResult {
    if !ctx.is_logged_in() {
        ctx.output.system("You must be logged in to log out.").await;
        return Ok(());
    }

    ctx.sess.write().logout();

    ctx.output.system("You have been logged out.").await;
    Ok(())
}
