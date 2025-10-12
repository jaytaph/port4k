use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::{failure, success};

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 3 {
        return Ok(failure!(super::USAGE));
    }

    let bp = &intent.args[2];
    let title = &intent.args[3];
    let account_id = ctx.account_id()?;

    if ctx.registry.services.blueprint.new_blueprint(bp, title, account_id).await? {
        Ok(success!(format!("[bp] created `{}`: {}\n", bp, title)))
    } else {
        Ok(failure!("[bp] already exists.\n"))
    }
}