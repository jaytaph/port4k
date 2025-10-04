//! @bp entry <bp>:<room>

use std::sync::Arc;
use anyhow::Result;
use crate::commands::blueprint::USAGE;
use crate::commands::{CmdCtx, CommandResult};
use crate::commands::CommandResult::{Failure, Success};
use crate::input::parser::Intent;
use crate::util::args::parse_bp_room_key;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() < 2 {
        return Ok(Failure(USAGE.into()));
    }

    let (bp, room) = match parse_bp_room_key(intent.args[1].as_str()) {
        Some((bp, room)) => (bp, room),
        None => return Ok(Failure("Invalid room key '{}'. Use <bp>:<room>\n".into())),
    };

    if !ctx.state.registry.services.blueprint.set_entry(&bp, &room).await? {
        return Ok(Failure("[bp] blueprint not found.\n".into()))
    }

    Ok(Success(format!("[bp] entry set: {}:{}\n", bp, room)))
}
