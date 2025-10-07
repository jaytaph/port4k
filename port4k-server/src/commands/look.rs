use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::input::parser::Intent;
use crate::rendering::{render_room, Theme};
use crate::services::CommandResult;
use crate::{failure, success};

pub async fn look(ctx: Arc<CmdCtx>, _intent: Intent) -> CommandResult<CommandOutput> {
    let (room_view, width) = {
        let s = ctx.sess.read();
        let c = match s.cursor.as_ref() {
            Some(c) => c,
            None => return Ok(failure!("You are nowhere.\n")),
        };

        let width = s.tty_cols.unwrap_or(80).max(20);

        let room_view = c.room.clone();
        (room_view, width)
    };

    Ok(success!(render_room(&Theme::blue(), width, room_view)))
}
