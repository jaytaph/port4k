#![allow(unused)]

use std::sync::Arc;
use crate::models::blueprint::Blueprint;
use crate::db::repo::room::RoomRepo;
use crate::error::ServiceError;
use crate::models::room::RoomView;
use crate::models::types::AccountId;
use crate::services::ServiceResult;

pub struct CursorService {
    repo: Arc<dyn RoomRepo>,
}

impl CursorService {
    pub fn new(repo: Arc<dyn RoomRepo>) -> Self {
        Self { repo }
    }

    pub fn enter_playtest(account_id: AccountId, blueprint: Blueprint) -> Cursor {
        let zone_ctx = ZoneContext::ephemeral(account_id, Arc::new(blueprint));

        let new_c = Cursor {
            zone_ctx,
            room: room_view,
        };
    }
}