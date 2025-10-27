#![allow(unused)]

use crate::commands::CmdCtx;
use crate::db::repo::{RoomRepo, ZoneRepo};
use crate::error::{AppResult, DomainError};
use crate::models::account::Account;
use crate::models::blueprint::Blueprint;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, RoomId};
use crate::models::zone::{Persistence, Zone, ZoneContext, ZoneKind, ZonePolicy};
use crate::services::RoomService;
use crate::state::session::Cursor;
use std::sync::Arc;

pub struct ZoneService {
    repo: Arc<dyn ZoneRepo>,
    room_service: Arc<RoomService>,
}

impl ZoneService {
    pub fn new(
        repo: Arc<dyn ZoneRepo>,
        room_service: Arc<RoomService>,
    ) -> Self {
        Self { repo, room_service }
    }

    pub async fn get_by_key(&self, zone_key: &str) -> AppResult<Option<Zone>> {
        Ok(self.repo.get_by_key(zone_key).await?)
    }

    /// Fetches the current zone context (saved in db?), or generates a new one if none exists.
    pub async fn generate_cursor(&self, ctx: Arc<CmdCtx>, account: &Account, room_id: RoomId) -> AppResult<Cursor> {
        // Get the room from the zone's blueprint to ensure it exists
        let zone_ctx = ctx.zone_ctx()?;

        let rv = self.room_service.build_room_view(&zone_ctx, account.id, room_id).await?;

        Ok(Cursor {
            zone_id: zone_ctx.zone.id,
            room_id,
            account_id: account.id,
            zone_ctx,
            room_view: rv,
        })
    }
}
