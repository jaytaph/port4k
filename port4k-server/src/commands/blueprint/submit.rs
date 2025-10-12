//! @bp submit <bp>

use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::{failure, success};
use crate::input::parser::Intent;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.is_empty() {
        return Ok(failure!(super::USAGE));
    }

    let bp = &intent.args[0];

    if ctx.registry.services.blueprint.submit(bp).await? {
        Ok(success!("[bp] submitted for review.\n"))
    } else {
        Ok(failure!("[bp] not found.\n"))
    }
}
