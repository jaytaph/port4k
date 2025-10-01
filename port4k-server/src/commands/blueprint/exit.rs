//! @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]

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
        "add" => {
            let (bp1, from) = parse_bp_room_key(&sub_args[0])
                .ok_or_else(|| anyhow::anyhow!("from must be <bp>:<room>"))?;
            let dir = sub_args[1].to_ascii_lowercase();
            let (bp2, to) = parse_bp_room_key(&sub_args[2])
                .ok_or_else(|| anyhow::anyhow!("to must be <bp>:<room>"))?;

            if bp1 != bp2 {
                return Ok(Failure("[bp] exits must stay within the same blueprint.\n".into()));
            }

            // Optional trailing "locked"
            let want_locked = sub_args.get(3)
                .map(|s| s.eq_ignore_ascii_case("locked"))
                .unwrap_or(false);

            let mut msg = String::new();

            if ctx.registry.db.bp_exit_add(&bp1, &from, &dir, &to).await? {
                msg.push_str(&format!("[bp] exit {}:{} --{}--> {} added.\n", bp1, from, dir, to));
            } else {
                msg.push_str("[bp] exit already exists.\n");
            }

            if want_locked {
                match ctx.registry.db.bp_room_set_locked(&bp1, &to, true).await {
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
