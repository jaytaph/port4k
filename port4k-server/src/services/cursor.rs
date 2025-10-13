#![allow(unused)]

use std::sync::Arc;
use crate::models::blueprint::Blueprint;
use crate::db::repo::room::RoomRepo;
use crate::error::AppResult;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, ScriptSource};
use crate::models::zone::{DbBackend, ZoneContext, ZoneRouter};
use crate::services::RoomService;
use crate::state::session::Cursor;

pub struct CursorService {
    router: Arc<ZoneRouter>,
    room_service: Arc<RoomService>,
}

impl CursorService {
    pub fn new(router: Arc<ZoneRouter>, room_service: Arc<RoomService>) -> Self {
        Self {
            router,
            room_service,
        }
    }

    pub async fn enter_playtest(&self, account_id: AccountId, blueprint: Blueprint) -> AppResult<Cursor> {
        let bp = Arc::new(blueprint);

        let zone_ctx = ZoneContext::ephemeral(account_id, bp.clone());
        let room_view = self.room_service.build_room_view(self.router.clone(), &zone_ctx, account_id, bp.entry_room_id).await?;

        Ok(Cursor {
            zone_ctx,
            room_id: bp.entry_room_id,
            room_view,
        })
    }
}