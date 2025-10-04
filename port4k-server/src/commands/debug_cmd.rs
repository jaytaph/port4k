use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use anyhow::Result;
use crate::commands::CommandResult::{Failure, Success};
use crate::input::parser::Intent;

pub async fn debug_cmd(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() < 2 {
        return Ok(Failure("Usage: debug where\n".into()));
    }

    let sub_cmd = intent.args[1].as_str();

    match sub_cmd {
        "where" => {
            let s = ctx.sess.read().unwrap();
            let username = s.account.as_ref().map(|a| a.username.as_str()).unwrap_or("[not logged in]");


            if let Some(cursor) = s.cursor.as_ref() {
                let msg = format!("[debug] user={username} zone={} zone_kind: {:?} room: {}\n", cursor.zone.title, cursor.zone_kind, cursor.room.room.title);
                return Ok(Success(msg));
            }

            Ok(Success(format!("[debug] user={username} not in a world\n")))
        },
        _ => Ok(Failure("Usage: @debug where\n".into()))
    }
}
