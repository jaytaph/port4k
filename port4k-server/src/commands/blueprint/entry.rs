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

    let (bp, room) = parse_bp_room_key(&intent.args[2])
        .ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;

    if !ctx.registry.repos.room.bp_set_entry(&bp, &room).await? {
        return Ok(Failure("[bp] blueprint not found.\n".into()))
    }

    Ok(Success(format!("[bp] entry set: {}:{}\n", bp, room)))
}
