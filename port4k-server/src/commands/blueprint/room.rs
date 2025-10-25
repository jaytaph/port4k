use crate::commands::{CmdCtx, CommandError, CommandResult};
use crate::input::parser::Intent;
use crate::util::args::parse_bp_room_key;
use std::sync::Arc;

const USAGE: &str = "Usage:
  @bp room add <bp>:<room> \"Title\" \"Body\"
  @bp room lock <bp>:<room>
  @bp room unlock <bp>:<room>\n";

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if intent.args.len() < 2 {
        ctx.output.system(USAGE).await;
        return Ok(());
    }

    let sub_cmd = &intent.args[1];
    let sub_args = &intent.args[2..];

    match sub_cmd.as_str() {
        // @bp room add <bp>:<room> "Title" "Body"
        "add" => {
            if sub_args.len() < 3 {
                ctx.output.system(USAGE).await;
                return Ok(());
            }

            let key = parse_bp_room_key(&sub_args[0]).ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            let title = &sub_args[1];
            let body = &sub_args[2];

            if title.is_empty() || body.is_empty() {
                ctx.output.system("[bp] title and body cannot be empty.").await;
                return Ok(());
            }

            if ctx.registry.services.blueprint.new_room(&key, title, body).await? {
                ctx.output.system(format!("[bp] room {}:{} added.\n", key.bp_key, key.room_key)).await;
            } else {
                ctx.output.system("[bp] room already exists").await;
            }

            Ok(())
        }

        // @bp room lock <bp>:<room>
        "lock" => {
            if sub_args.is_empty() {
                ctx.output.system(USAGE).await;
                return Ok(());
            }

            let key = parse_bp_room_key(&sub_args[0]).ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            if ctx.registry.services.blueprint.set_locked(&key, true).await? {
                ctx.output.system("[bp] blueprint/room set to LOCKED.").await;
            } else {
                ctx.output.system("[bp] blueprint/room not found.").await;
            }
            Ok(())
        }

        // @bp room unlock <bp>:<room>
        "unlock" => {
            if sub_args.is_empty() {
                ctx.output.system(USAGE).await;
                return Ok(());
            }

            let key = parse_bp_room_key(&sub_args[0]).ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            if ctx.registry.services.blueprint.set_locked(&key, false).await? {
                ctx.output.system("[bp] blueprint/room set to UNLOCKED.").await;
            } else {
                ctx.output.system("[bp] blueprint/room not found.").await;
            }
            Ok(())
        }

        _ => {
            ctx.output.system(super::USAGE).await;
            Ok(())
        }
    }
}
