use std::sync::Arc;
use anyhow::Result;
use crate::commands::{CmdCtx, CommandResult};
use crate::commands::CommandResult::{Failure, Success};
use crate::input::parser::Intent;
use crate::util::args::parse_bp_room_key;

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() < 4 {
        return Ok(Failure(super::USAGE.into()));
    }

    let sub_cmd = intent.args[1].as_str();
    let sub_args = intent.args[2..].to_vec();

    match sub_cmd {
        // @bp room add <bp>:<room> "Title" "Body"
        "add" if sub_args.len() >= 3 => {
            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| anyhow::anyhow!("room key must be <bp>:<room>"))?;
            let title = &sub_args[1];
            let body  = &sub_args[2];

            if ctx.registry.db.bp_room_add(&bp, &room, title, body).await? {
                Ok(Success(format!("[bp] room {}:{} added.\n", bp, room)))
            } else {
                Ok(Failure("[bp] room already exists.\n".into()))
            }
        }

        // @bp room lock <bp>:<room>
        "lock" if sub_args.len() >= 2 => {
            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
            if ctx.registry.db.bp_room_set_locked(&bp, &room, true).await? {
                Ok(Success(format!("[bp] room {}:{} set to LOCKED.\n", bp, room)))
            } else {
                Ok(Failure("[bp] blueprint/room not found.\n".into()))
            }
        }

        // @bp room unlock <bp>:<room>
        "unlock" if sub_args.len() >= 2 => {
            let (bp, room) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
            if ctx.registry.db.bp_room_set_locked(&bp, &room, false).await? {
                Ok(Success(format!("[bp] room {}:{} set to UNLOCKED.\n", bp, room)))
            } else {
                Ok(Failure("[bp] blueprint/room not found.\n".into()))
            }
        }

        _ => Ok(Failure(super::USAGE.into())),
    }
}