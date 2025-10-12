use std::sync::Arc;
use crate::commands::{CmdCtx, CommandError, CommandOutput, CommandResult};
use crate::{failure, success};
use crate::input::parser::Intent;
use crate::util::args::parse_bp_room_key;

const USAGE: &str = "Usage:
  @bp room add <bp>:<room> \"Title\" \"Body\"
  @bp room lock <bp>:<room>
  @bp room unlock <bp>:<room>\n";

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 2 {
        return Ok(failure!(USAGE));
    }

    let sub_cmd = &intent.args[1];
    let sub_args = &intent.args[2..];

    match sub_cmd.as_str() {
        // @bp room add <bp>:<room> "Title" "Body"
        "add" => {
            if sub_args.len() < 3 {
                return Ok(failure!(USAGE));
            }

            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            let title = &sub_args[1];
            let body  = &sub_args[2];

            if title.is_empty() || body.is_empty() {
                return Ok(failure!("[bp] title and body cannot be empty.\n"));
            }

            if ctx.registry.services.blueprint.new_room(&bp, &room, title, body).await? {
                Ok(success!(format!("[bp] room {}:{} added.\n", bp, room)))
            } else {
                Ok(failure!("[bp] room already exists.\n"))
            }
        }

        // @bp room lock <bp>:<room>
        "lock" => {
            if sub_args.len() < 1 {
                return Ok(failure!(USAGE));
            }

            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            if ctx.registry.services.blueprint.set_locked(&bp, &room, true).await? {
                Ok(success!(format!("[bp] room {}:{} set to LOCKED.\n", bp, room)))
            } else {
                Ok(failure!("[bp] blueprint/room not found.\n"))
            }
        }

        // @bp room unlock <bp>:<room>
        "unlock" => {
            if sub_args.len() < 1 {
                return Ok(failure!(USAGE));
            }

            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            if ctx.registry.services.blueprint.set_locked(&bp, &room, false).await? {
                Ok(success!(format!("[bp] room {}:{} set to UNLOCKED.\n", bp, room)))
            } else {
                Ok(failure!("[bp] blueprint/room not found.\n"))
            }
        }

        _ => Ok(failure!(super::USAGE)),
    }
}