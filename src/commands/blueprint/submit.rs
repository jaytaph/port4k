//! @bp submit <bp>

use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;

#[allow(unused)]
pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if intent.args.is_empty() {
        ctx.output.system(super::USAGE).await;
        return Ok(());
    }

    let bp = &intent.args[0];

    if ctx.registry.services.blueprint.submit(bp).await? {
        ctx.output.system("[bp] submitted for review.").await;
    } else {
        ctx.output.system("[bp] submission failed").await;
    }
    Ok(())
}
