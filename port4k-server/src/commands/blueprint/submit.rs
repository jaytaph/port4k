//! @bp submit <bp>

use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();
    if intent.args.is_empty() {
        out.append(super::USAGE);
        out.failure();
        return Ok(out);
    }

    let bp = &intent.args[0];

    if ctx.registry.services.blueprint.submit(bp).await? {
        out.append("[bp] submitted for review.\n");
        out.success();
    } else {
        out.append("[bp] submission failed: ");
        out.failure();
    }
    Ok(out)
}
