use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() < 3 {
        out.append(super::USAGE);
        out.failure();
        return Ok(out);
    }

    let bp = &intent.args[2];
    let title = &intent.args[3];
    let account_id = ctx.account_id()?;

    if ctx.registry.services.blueprint.new_blueprint(bp, title, account_id).await? {
        out.append(format!("[bp] created `{}`: {}\n", bp, title).as_str());
        out.success();
    } else {
        out.append("[bp] already exists.\n");
        out.failure();
    }

    Ok(out)
}