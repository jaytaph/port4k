use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::rendering::{render_room, Theme};
use crate::success;
use crate::error::DomainError;

pub async fn look(ctx: Arc<CmdCtx>, _intent: Intent) -> CommandResult<CommandOutput> {
    let zone_ctx = ctx.zone_ctx().map_err(|_| DomainError::NotFound)?;
    let account = ctx.account().map_err(|_| DomainError::NotLoggedIn)?;
    let cursor = ctx.cursor().map_err(|_| DomainError::NotFound)?;

    let room_view = ctx.registry.services.room.create_view(&zone_ctx, &account, &cursor).await?;

    Ok(success!(render_room(&Theme::blue(), 80, room_view)))
}
