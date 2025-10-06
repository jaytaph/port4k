//! @bp submit <bp>

use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::commands::CommandResult::{Failure, Success};
use crate::error::AppResult;
use crate::input::parser::Intent;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> AppResult<CommandResult> {
    if intent.args.is_empty() {
        return Ok(Failure(super::USAGE.into()));
    }

    let bp = &intent.args[0];

    if ctx.state.registry.services.blueprint.submit(bp).await? {
        Ok(Success("[bp] submitted for review.\n".into()))
    } else {
        Ok(Failure("[bp] not found.\n".into()))
    }
}
