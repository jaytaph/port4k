use crate::commands::{CmdCtx, CommandError, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::util::args::parse_bp_room_key;
use std::sync::Arc;

const USAGE: &str = "Usage:
  @bp room add <bp>:<room> \"Title\" \"Body\"
  @bp room lock <bp>:<room>
  @bp room unlock <bp>:<room>\n";

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() < 2 {
        out.append(USAGE);
        out.failure();
        return Ok(out);
    }

    let sub_cmd = &intent.args[1];
    let sub_args = &intent.args[2..];

    match sub_cmd.as_str() {
        // @bp room add <bp>:<room> "Title" "Body"
        "add" => {
            if sub_args.len() < 3 {
                out.append(USAGE);
                out.failure();
                return Ok(out);
            }

            let key = parse_bp_room_key(&sub_args[0]).ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            let title = &sub_args[1];
            let body = &sub_args[2];

            if title.is_empty() || body.is_empty() {
                out.append("[bp] title and body cannot be empty.\n");
                out.failure();
                return Ok(out);
            }

            if ctx.registry.services.blueprint.new_room(&key, title, body).await? {
                out.append(format!("[bp] room {}:{} added.\n", key.bp_key, key.room_key).as_str());
                out.success();
            } else {
                out.append("[bp] room already exists.\n");
                out.failure();
            }

            Ok(out)
        }

        // @bp room lock <bp>:<room>
        "lock" => {
            if sub_args.is_empty() {
                out.append(USAGE);
                out.failure();
                return Ok(out);
            }

            let key = parse_bp_room_key(&sub_args[0]).ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            if ctx.registry.services.blueprint.set_locked(&key, true).await? {
                out.append("[bp] blueprint/room set to LOCKED.\n");
                out.success();
            } else {
                out.append("[bp] blueprint/room not found.\n");
                out.failure();
            }
            Ok(out)
        }

        // @bp room unlock <bp>:<room>
        "unlock" => {
            if sub_args.is_empty() {
                out.append(USAGE);
                out.failure();
                return Ok(out);
            }

            let key = parse_bp_room_key(&sub_args[0]).ok_or_else(|| CommandError::Custom("use <bp>:<room>".into()))?;

            if ctx.registry.services.blueprint.set_locked(&key, false).await? {
                out.append("[bp] blueprint/room set to UNLOCKED.\n");
                out.success();
            } else {
                out.append("[bp] blueprint/room not found.\n");
                out.failure();
            }
            Ok(out)
        }

        _ => {
            out.append(super::USAGE);
            out.failure();
            Ok(out)
        }
    }
}
