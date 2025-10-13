//! @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]

use std::sync::Arc;
use crate::commands::{CmdCtx, CommandError, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::{failure, success};
use crate::util::args::{normalize_dir, parse_bp_room_key};

pub async fn run(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    const USAGE: &str = "Usage:\n  @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]\n";

    // args layout: [ "@bp", "exit", <sub_cmd>, ... ]
    let [_, _, sub_cmd, rest @ ..] = &*intent.args else {
        return Ok(failure!(USAGE));
    };

    match sub_cmd.as_str() {
        "add" => {
            // expect: <from_key> <dir> <to_key> [locked]
            let [from_key, dir_raw, to_key, tail @ ..] = rest else {
                return Ok(failure!(USAGE));
            };

            // parse & validate inputs (use `?` with precise errors)
            let from_key = parse_bp_room_key(from_key)
                .ok_or(CommandError::Custom("from must be <bp>:<room>".into()))?;
            let dir = normalize_dir(dir_raw)
                .ok_or(CommandError::Custom(
                    "dir must be a valid direction (n, ne, e, se, s, sw, w, nw, up, down)".into()
                ))?;
            let to_key = parse_bp_room_key(to_key)
                .ok_or(CommandError::Custom("to must be <bp>:<room>".into()))?;

            if from_key.bp_key != to_key.bp_key {
                return Ok(failure!("[bp] exits must stay within the same blueprint.\n"));
            }

            // optional trailing "locked"
            let want_locked = tail.first().map(|s| s.eq_ignore_ascii_case("locked")).unwrap_or(false);

            // build response text
            use std::fmt::Write as _;
            let mut msg = String::new();

            if ctx.registry.services.blueprint.add_exit(&from_key, &dir, &to_key).await? {
                let _ = writeln!(&mut msg, "[bp] exit {}:{} --{}--> {} added.", from_key.bp_key, from_key.room_key, dir, to_key.room_key);
            } else {
                let _ = writeln!(&mut msg, "[bp] exit already exists.");
            }

            if want_locked {
                match ctx.registry.services.blueprint.set_locked(&to_key, true).await {
                    Ok(true)  => { let _ = writeln!(&mut msg, "[bp] room {}:{} set to LOCKED.", to_key.bp_key, to_key.room_key); },
                    Ok(false) => { let _ = writeln!(&mut msg, "[bp] could not lock destination (room not found?)."); }
                    Err(e)    => { let _ = writeln!(&mut msg, "[bp] failed to lock destination: {}", e); }
                }
            }

            Ok(success!(msg))
        }

        _ => Ok(failure!(USAGE)),
    }
}
