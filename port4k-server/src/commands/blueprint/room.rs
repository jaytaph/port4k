use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::error::AppError;
use crate::input::parser::Intent;
use crate::services::CommandResult;
use crate::util::args::parse_bp_room_key;

const USAGE: &str = "Usage:
  @bp room add <bp>:<room> \"Title\" \"Body\"
  @bp room lock <bp>:<room>
  @bp room unlock <bp>:<room>\n";

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 2 {
        return Ok(Failure(USAGE.into()));
    }

    let sub_cmd = &intent.args[1];
    let sub_args = &intent.args[2..];

    match sub_cmd.as_str() {
        // @bp room add <bp>:<room> "Title" "Body"
        "add" => {
            if sub_args.len() < 3 {
                return Ok(Failure(USAGE.into()));
            }

            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| AppError::Args("room key must be <bp>:<room>"))?;

            let title = &sub_args[1];
            let body  = &sub_args[2];

            if title.is_empty() || body.is_empty() {
                return Ok(Failure("[bp] title and body cannot be empty.\n".into()));
            }

            if ctx.state.registry.services.blueprint.new_room(&bp, &room, title, body).await? {
                Ok(Success(format!("[bp] room {}:{} added.\n", bp, room)))
            } else {
                Ok(Failure("[bp] room already exists.\n".into()))
            }
        }

        // @bp room lock <bp>:<room>
        "lock" => {
            if sub_args.len() < 1 {
                return Ok(Failure(USAGE.into()));
            }

            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;

            if ctx.state.registry.services.blueprint.set_locked(&bp, &room, true).await? {
                Ok(Success(format!("[bp] room {}:{} set to LOCKED.\n", bp, room)))
            } else {
                Ok(Failure("[bp] blueprint/room not found.\n".into()))
            }
        }

        // @bp room unlock <bp>:<room>
        "unlock" => {
            if sub_args.len() < 1 {
                return Ok(Failure(USAGE.into()));
            }

            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;

            if ctx.state.registry.services.blueprint.set_locked(&bp, &room, false).await? {
                Ok(Success(format!("[bp] room {}:{} set to UNLOCKED.\n", bp, room)))
            } else {
                Ok(Failure("[bp] blueprint/room not found.\n".into()))
            }
        }

        _ => Ok(Failure(super::USAGE.into())),
    }
}