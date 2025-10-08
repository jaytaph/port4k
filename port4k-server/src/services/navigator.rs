use std::sync::Arc;
use crate::error::AppResult;
use crate::models::types::{AccountId, Direction, RoomId};
use crate::models::zone::{ZoneContext, ZoneRouter};

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
        zone_ctx: &ZoneContext,
        account_id: AccountId,
        dir: Direction,
    ) -> AppResult<(RoomId, RoomId)> {
        // 1) read current room via state repo (routed by policy)
        let state_repo = self.zone_router.state_repo_for(zone_ctx);
        let from = state_repo.current_room(zone_ctx, account_id).await?;

        // 2) resolve exit + rules (replace with your real world/exits)
        let (_exit, to) = self.resolve_exit_checked(zone_ctx, from, dir).await?;

        // 3) begin UoW (routed by policy); persist move + audit atomically
        let backend = self.zone_router.backend_for(zone_ctx);
        let mut uow = backend.begin(zone_ctx).await?;

        uow.set_current_room(account_id, to).await?;
        uow.record_travel(account_id, from, to).await?;

        // Example: award XP on first visit, or health check, etc.
        // uow.update_xp(account_id, 5).await?;

        uow.commit().await?;
        Ok((from, to))
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