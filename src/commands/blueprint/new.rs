use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;

#[allow(unused)]
pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if intent.args.len() < 3 {
        ctx.output.system(super::USAGE).await;
        return Ok(());
    }

    let bp = &intent.args[2];
    let title = &intent.args[3];
    let account_id = ctx.account_id()?;

    if ctx
        .registry
        .services
        .blueprint
        .new_blueprint(bp, title, account_id)
        .await?
    {
        ctx.output.system(format!("[bp] created `{}`: {}", bp, title)).await;
    } else {
        ctx.output.system("[bp] already exists.").await;
    }

    Ok(())
}
