//! @bp entry <bp>:<room>

use std::sync::Arc;
use crate::commands::blueprint::USAGE;
use crate::commands::{CmdCtx, CommandError, CommandOutput, CommandResult};
use crate::{failure, success};
use crate::input::parser::Intent;
use crate::util::args::parse_bp_room_key;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 2 {
        return Ok(failure!(USAGE));
    }

    let (bp_key, room_key) = parse_bp_room_key(intent.args[1].as_str())
        .ok_or(CommandError::Custom("invalid room key. use <bp>:<room>".into()))?;

    if !ctx.registry.services.blueprint.set_entry(&bp_key, &room_key).await? {
        return Ok(failure!("[bp] blueprint not found.\n"))
    }

    Ok(success!(format!("[bp] entry set: {}:{}\n", bp_key, room_key)))
}
