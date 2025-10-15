//! @bp entry <bp>:<room>

use std::sync::Arc;
use crate::commands::blueprint::USAGE;
use crate::commands::{CmdCtx, CommandError, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::util::args::parse_bp_room_key;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() < 2 {
        out.append(USAGE);
        out.failure();
        return Ok(out);
    }

    let key = parse_bp_room_key(intent.args[1].as_str())
        .ok_or(CommandError::Custom("invalid room key. use <bp>:<room>".into()))?;

    if !ctx.registry.services.blueprint.set_entry(&key).await? {
        out.append("Blueprint or room not found.\n");
        out.failure();
        return Ok(out);
    }

    out.append("Blueprint entry set.\n");
    out.success();
    Ok(out)
}
