use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::{failure, success};
use crate::input::parser::Intent;

pub async fn debug_cmd(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 2 {
        return Ok(failure!("Usage: debug where\n"));
    }

    let sub_cmd = intent.args[1].as_str();

    match sub_cmd {
        "where" => {
            let account = ctx.account()?;
            let username = account.username;

            if ! ctx.has_cursor() {
                return Ok(failure!("You have no cursor. Use 'go <zone>' to set one.\n"));
            }

            let cursor = ctx.cursor()?;
            let msg = format!("[debug] user={username} zone={} zone_kind: {:?} room: {}\n", cursor.zone_ctx.zone.title, cursor.zone_ctx.kind, cursor.room_view.room.title);
            Ok(success!(msg))
        },
        _ => Ok(failure!("Usage: @debug where\n"))
    }
}
