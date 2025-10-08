#![allow(unused)]

use std::sync::Arc;
use crate::models::blueprint::Blueprint;
use crate::db::repo::room::RoomRepo;
use crate::error::AppResult;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, ScriptSource};
use crate::models::zone::ZoneContext;
use crate::state::session::Cursor;

pub struct CursorService {
    repo: Arc<dyn RoomRepo>,
}

impl CursorService {
    pub fn new(repo: Arc<dyn RoomRepo>) -> Self {
        Self { repo }
    }

    pub async fn enter_playtest(&self, account_id: AccountId, blueprint: Blueprint) -> AppResult<Cursor> {
        let bp = Arc::new(blueprint);
        let zone_ctx = ZoneContext::ephemeral(account_id, bp.clone());
        let room_view = self.repo.get_view(bp.entry_room_id, None, ScriptSource::Live).await?;

        Ok(Cursor {
            zone_ctx,
            room: room_view,
        })
    }
}