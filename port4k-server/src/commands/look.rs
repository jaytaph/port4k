use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use anyhow::{anyhow, Result};
use crate::commands::CommandResult::{Failure, Success};
use crate::rendering::{render_room, Theme};

pub async fn look(ctx: Arc<CmdCtx>, _intent: Intent) -> Result<CommandResult> {
    let (room_view, width) = {
        let s = ctx.sess.read().map_err(|_| anyhow!("Session lock poisoned"))?;
        let c = match s.cursor.as_ref() {
            Some(c) => c,
            None => return Ok(Failure("You are nowhere.\n".into())),
        };

        let width = s.tty_cols.unwrap_or(80).max(20);

        let room_view = c.room.clone();
        (room_view, width)
    };

    Ok(Success(render_room(&Theme::blue(), width, room_view)))
}
