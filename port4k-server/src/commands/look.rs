use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::renderer::{render_room, Theme};
use crate::success;
use crate::error::DomainError;

pub async fn look(ctx: Arc<CmdCtx>, _intent: Intent) -> CommandResult<CommandOutput> {
    let cursor = ctx.cursor().map_err(|_| DomainError::NotFound)?;

    Ok(success!(render_room(&Theme::blue(), 80, cursor.room_view)))
}
