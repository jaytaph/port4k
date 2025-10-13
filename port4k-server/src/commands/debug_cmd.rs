use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;

pub async fn debug_cmd(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() < 2 {
        out.append("Usage: debug where\n");
        out.failure();
        return Ok(out);
    }

    let sub_cmd = intent.args[1].as_str();

    match sub_cmd {
        "where" => {
            let account = ctx.account()?;
            let username = account.username;

            if ! ctx.has_cursor() {
                out.append("You have no cursor. Use 'go <zone>' to set one.\n");
                out.failure();
            }

            let cursor = ctx.cursor()?;
            out.append(format!("[debug] user={username} zone={} zone_kind: {:?} room: {}\n", cursor.zone_ctx.zone.title, cursor.zone_ctx.kind, cursor.room_view.room.title).as_str());
            out.success();
        },
        _ => {
            out.append("Unknown debug command.\n");
            out.append("Available commands: where\n");
            out.failure();
        }
    }

    Ok(out)
}
