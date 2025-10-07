//! @bp submit <bp>

use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::input::parser::Intent;
use crate::services::CommandResult;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.is_empty() {
        return Ok(failure!(super::USAGE.into()));
    }

    let bp = &intent.args[0];

    if ctx.state.registry.services.blueprint.submit(bp).await? {
        Ok(success!("[bp] submitted for review.\n".into()))
    } else {
        Ok(failure!("[bp] not found.\n".into()))
    }
}
