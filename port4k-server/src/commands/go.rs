use std::sync::Arc;
use crate::commands::{CmdCtx, CommandError, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::{success, ConnState};
use crate::error::DomainError;
use crate::models::types::Direction;
use crate::renderer::{render_room, Theme};

pub async fn go(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if !ctx.has_cursor() {
        return Err(CommandError::Domain(DomainError::NoCurrentRoom));
    }

    let Some(dir) = intent.direction else {
        return Err(CommandError::InvalidArgs("No direction specified".to_string()));
    };
    let dir = Direction::from(dir);

    let c = ctx.cursor()?;
    let account_id = ctx.account_id()?;
    let (_from, _to) = ctx.registry.services.navigator.go(&c, account_id, dir).await?;

    // // reuse your render path
    // let view_repo = ctx.registry.services.zone_router.view_repo_for(&ctx.zone_ctx);
    // let room_view = view_repo.room_view(&ctx.zone_ctx, to, ctx.screen_width).await?;
    // let text = render_room(&room_view);

    let text = "we moved to another room".to_string();

    Ok(success!(text))
}
