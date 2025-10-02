use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::state::session::WorldMode;
use anyhow::Result;
use crate::commands::CommandResult::{Failure, Success};
use crate::input::parser::Intent;

pub async fn debug_cmd(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() < 2 {
        return Ok(Failure("Usage: debug where\n".into()));
    }

    let sub_cmd = intent.args[1].as_str();
    // let sub_args = intent.args[2..].to_vec();

    match sub_cmd {
        "where" => {
            let s = ctx.sess.read().unwrap();
            let username = s.account.as_ref().map(|a| a.username.as_str()).unwrap_or("[not logged in]");
            let msg = match &s.world {
                Some(WorldMode::Live { room_id }) => {
                    format!("[debug] user={username} world=Live room_id={}\n", room_id)
                }
                Some(WorldMode::Playtest { bp, room, .. }) => {
                    format!("[debug] user={username} world=Playtest {}:{}\n", bp, room)
                }
                None => format!("[debug] user={username} world=None\n"),
            };
            Ok(Success(msg))
        }
        _ => Ok(Failure("Usage: @debug where\n".into())),
    }
}
