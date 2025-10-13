use std::sync::Arc;
use crate::error::{AppResult, DomainError};
use crate::models::types::{AccountId, Direction, RoomId};
use crate::models::zone::ZoneRouter;
use crate::state::session::Cursor;

#[allow(unused)]
#[derive(Clone)]
struct ResolvedExit {
    from: RoomId,
    to_room: RoomId,
    locked_message: Option<String>,
}

pub struct NavigatorService {
    pub zone_router: Arc<ZoneRouter>,
}

impl NavigatorService {
    pub fn new(zone_router: Arc<ZoneRouter>) -> Self {
        Self { zone_router }
    }

    pub async fn go(
        &self,
        cursor: &Cursor,
        account_id: AccountId,
        dir: Direction,
    ) -> AppResult<(RoomId, RoomId)> {
        let from_id = cursor.room_view.room.id;
        let (_exit, to_id) = self.resolve_exit_checked(&cursor.zone_ctx, from_id, dir).await?;

        let state = self.zone_router.state_for(&cursor.zone_ctx);

        let mut uow = state.begin(&cursor.zone_ctx).await?;
        uow.set_current_room(account_id, to_id).await?;
        uow.record_travel(account_id, from_id, to_id).await?;
        // uow.update_xp(account_id, 5).await?;
        uow.commit().await?;

        Ok((from_id, to_id))
    }

    async fn resolve_exit_checked(
        &self,
        _zone_ctx: &ZoneContext,
        from: RoomId,
        _dir: Direction,
    ) -> AppResult<(ResolvedExit, RoomId)> {
        // TODO: look up exit, check locked, lua hooks, visibility, etc.
        // Return Err(AppError::user("You can’t go that way")) on failure.
        let exit = ResolvedExit { from, to_room: RoomId::new(), locked_message: None };
        Ok((exit.clone(), exit.to_room))
    }
}