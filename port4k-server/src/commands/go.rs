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
    let account = ctx.account()?;
    let (from_id, to_id) = ctx.registry.services.navigator.go(&c, account.id, dir).await?;

    let c = ctx.registry.services.zone.generate_cursor(ctx.clone(), &account, RoomKey::Id(from_id), RoomKey::Id(to_id)).await?;
    {
        let mut s = ctx.sess.write();
        s.account = Some(account.clone());  // @TODO: Is this wise? Why clone?
        s.state = ConnState::LoggedIn;
        s.cursor = Some(c);
    }


    // // reuse your render path
    // let view_repo = ctx.registry.services.zone_router.view_repo_for(&ctx.zone_ctx);
    // let room_view = view_repo.room_view(&ctx.zone_ctx, to, ctx.screen_width).await?;
    // let text = render_room(&room_view);

    Ok(success!(render_room(&Theme::blue(), 80, c.room_view)))
}
