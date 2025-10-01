use std::sync::Arc;
use anyhow::Result;
use crate::commands::{CmdCtx, CommandResult};
use crate::commands::blueprint::utils::current_owner;
use crate::commands::CommandResult::{Failure, Success};
use crate::input::parser::Intent;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() < 3 {
        return Ok(Failure(super::USAGE.into()));
    }

    let bp = &intent.args[2];
    let title = &intent.args[3];
    let owner = current_owner(ctx.clone())?;

    if ctx.registry.db.bp_new(bp, title, &owner).await? {
        Ok(Success(format!("[bp] created `{}`: {}\n", bp, title)))
    } else {
        Ok(Failure("[bp] already exists.\n".into()))
    }
}