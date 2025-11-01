//! @bp entry <bp>:<room>

use crate::commands::blueprint::USAGE;
use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::util::args::parse_bp_room_key;
use std::sync::Arc;

#[allow(unused)]
pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if intent.args.len() < 2 {
        ctx.output.system(USAGE).await;
        return Ok(());
    }

    let Some(key) = parse_bp_room_key(intent.args[1].as_str()) else {
        ctx.output.system("invalid room key. use <bp>:<room>").await;
        return Ok(());
    };

    if !ctx.registry.services.blueprint.set_entry(&key).await? {
        ctx.output.system("Blueprint or room not found.").await;
        return Ok(());
    }

    ctx.output.system("Blueprint entry set.").await;
    Ok(())
}
