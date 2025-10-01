//! @bp submit <bp>

use std::sync::Arc;
use anyhow::Result;
use crate::commands::{CmdCtx, CommandResult};
use crate::commands::CommandResult::{Failure, Success};
use crate::input::parser::Intent;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.is_empty() {
        return Ok(Failure(super::USAGE.into()));
    }
    let bp = &intent.args[0];

    let client = ctx.registry.db.pool.get().await?;
    let n = client
        .execute("UPDATE blueprints SET status='submitted' WHERE key=$1", &[bp])
        .await?;

    if n == 1 {
        Ok(Success("[bp] submitted for review.\n".into()))
    } else {
        Ok(Failure("[bp] not found.\n".into()))
    }
}
