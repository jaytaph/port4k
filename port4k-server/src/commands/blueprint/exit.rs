//! @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]

use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::error::AppResult;
use crate::input::parser::Intent;
use crate::util::args::{normalize_dir, parse_bp_room_key};

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> AppResult<CommandOutput> {
    if intent.args.len() < 5 {
        return Ok(Failure(super::USAGE.into()));
    }

    let sub_cmd = intent.args[1].as_str();
    let sub_args = &intent.args[2..];

    match sub_cmd {
        "add" => {
            if sub_args.len() < 3 {
                return Ok(Failure("Usage: @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]\n".into()));
            }

            let from_key = &sub_args[0];
            let dir_raw = &sub_args[1];
            let to_key = &sub_args[2];

            let (bp1, from) = parse_bp_room_key(&from_key)
                .ok_or_else(|| anyhow::anyhow!("from must be <bp>:<room>"))?;
            let dir = normalize_dir(&dir_raw)
                .ok_or_else(|| anyhow::anyhow!("dir must be a valid direction (n, ne, e, se, s, sw, w, nw, up, down)"))?;
            let (bp2, to) = parse_bp_room_key(&to_key)
                .ok_or_else(|| anyhow::anyhow!("to must be <bp>:<room>"))?;

            if bp1 != bp2 {
                return Ok(Failure("[bp] exits must stay within the same blueprint.\n".into()));
            }

            // Optional trailing "locked"
            let want_locked = sub_args.get(3)
                .map(|s| s.eq_ignore_ascii_case("locked"))
                .unwrap_or(false);

            let mut msg = String::new();

            if ctx.state.registry.services.blueprint.add_exit(&bp1, &from, &dir, &to).await? {
                msg.push_str(&format!("[bp] exit {}:{} --{}--> {} added.\n", bp1, from, dir, to));
            } else {
                msg.push_str("[bp] exit already exists.\n");
            }

            if want_locked {
                match ctx.state.registry.services.blueprint.set_locked(&bp1, &to, true).await {
                    Ok(true)  => msg.push_str(&format!("[bp] room {}:{} set to LOCKED.\n", bp1, to)),
                    Ok(false) => msg.push_str("[bp] could not lock destination (room not found?).\n"),
                    Err(e)    => msg.push_str(&format!("[bp] failed to lock destination: {}\n", e)),
                }
            }
            Ok(Success(msg))
        }
        _ => Ok(Failure("Usage:\n  @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]\n".into())),
    }
}
