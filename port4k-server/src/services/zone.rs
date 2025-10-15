#![allow(unused)]

use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::models::blueprint::Blueprint;
use crate::db::repo::room::RoomRepo;
use crate::db::repo::zone::ZoneRepo;
use crate::error::{AppResult, DomainError};
use crate::models::account::Account;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, RoomId};
use crate::models::zone::{Persistence, Zone, ZoneContext, ZoneKind, ZonePolicy};
use crate::services::RoomService;
use crate::state::session::Cursor;

pub struct ZoneService {
    repo: Arc<dyn ZoneRepo>,
}

impl ZoneService {
    pub fn new(repo: Arc<dyn ZoneRepo>) -> Self {
        Self { repo }
    }

    pub async fn get_by_key(&self, zone_key: &str) -> AppResult<Option<Zone>> {
        Ok(self.repo.get_by_key(zone_key).await?)
    }

    /// Fetches the current zone context (saved in db?), or generates a new one if none exists.
    pub async fn generate_cursor(&self, ctx: Arc<CmdCtx>, account: &Account, room_id: RoomId) -> AppResult<Cursor> {
        // Get the room from the zone's blueprint to ensure it exists
        let zone_ctx = ctx.zone_ctx()?;
        let room = ctx.registry.services.blueprint.room_by_id(zone_ctx.blueprint.id, room_id).await?;

        // Generate the new room view for given account, zone(_ctx) and room
        let room_view = ctx.registry.services.room.build_room_view(
            ctx.registry.zone_router.clone(),
            &zone_ctx,
            account.id,
            room_id,
        ).await?;

        Ok(Cursor { zone_ctx, room_id, room_view })
    }
}