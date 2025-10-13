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

enum RoomKey {
    Id(RoomId),
    Key(String),
}

impl ZoneService {
    pub fn new(repo: Arc<dyn ZoneRepo>) -> Self {
        Self { repo }
    }

    pub async fn get_by_key(&self, zone_key: &str) -> AppResult<Option<Zone>> {
        Ok(self.repo.get_by_key(zone_key).await?)
    }

    /// Fetches the current zone context (saved in db?), or generates a new one if none exists.
    pub async fn generate_cursor(&self, ctx: Arc<CmdCtx>, account: &Account, zone_key: Option<&str>, bp_room_key: RoomKey) -> AppResult<Cursor> {
        let blueprint = Arc::new(ctx.registry.services.blueprint.get_by_key(bp_key.unwrap_or("hub")).await?);

        match bp_room_key {
            RoomKey::Id(id) => {
                let room = ctx.registry.services.room.get_by_id(id).await?;
            },
            RoomKey::Key(key) => {
                let room = ctx.registry.services.room.get_by_key(key).await?;
            }
        }
        let Some(zone) = ctx.registry.services.zone.get_by_key(zone_key.unwrap_or("hub")).await? else {
            return Err(DomainError::NotFound.into());
        };

        let zone_ctx = ZoneContext{
            zone: Arc::new(zone),
            kind: ZoneKind::Live,
            policy: ZonePolicy {
                persistence: Persistence::Persistent,
            },
            blueprint: blueprint.clone(),
        };

        let room_view = ctx.registry.services.room.build_room_view(
            ctx.registry.zone_router.clone(),
            &zone_ctx,
            account.id,
            blueprint.entry_room_id,
        ).await?;

        Ok(Cursor { zone_ctx, room_view })
    }
}