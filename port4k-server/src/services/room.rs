use std::sync::Arc;
use tokio::try_join;
use crate::db::repo::kv::KvRepo;
use crate::error::AppResult;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, RoomId};
use crate::models::zone::{ZoneBackend, ZoneContext, ZoneRouter, ZoneStateRepo};
use crate::services::BlueprintService;

pub struct RoomService {
    repo: Arc<dyn KvRepo>,
    blueprint_service: Arc<BlueprintService>,
}

impl RoomService {
    pub fn new(
        repo: Arc<dyn KvRepo>,
        blueprint_service: Arc<BlueprintService>,
    ) -> Self {
        Self { repo, blueprint_service }
    }

    /// Get room key-value data
    pub async fn room_kv_get(&self, room_id: RoomId, object_key: &str) -> AppResult<serde_json::Value> {
        let v = self.repo.room_kv_get(room_id, object_key).await?;
        Ok(v)
    }

    /// Set room key-value data
    pub async fn room_kv_set(&self, room_id: RoomId, object_key: &str, v: &serde_json::Value) -> AppResult<()> {
        self.repo.room_kv_set(room_id, object_key, v.clone()).await?;
        Ok(())
    }

    /// Get player key-value data for specific room
    pub async fn player_kv_get(&self, room_id: RoomId, account_id: AccountId, object_key: &str) -> AppResult<Option<serde_json::Value>> {
        let v = self.repo.player_kv_get(room_id, account_id, object_key).await?;
        Ok(v)
    }

    /// Set player key-value data for specific room
    pub async fn player_kv_set(&self, room_id: RoomId, account_id: AccountId, object_key: &str, v: &serde_json::Value) -> AppResult<()> {
        self.repo.player_kv_set(room_id, account_id, object_key, v.clone()).await?;
        Ok(())
    }

    /// Creates a RoomView for the given cursor in the given zone context
    pub async fn build_room_view(router: Arc<ZoneRouter>, zone_ctx: &ZoneContext, account_id: AccountId, room_id: RoomId) -> AppResult<RoomView> {
        let backend: Arc<dyn ZoneBackend> = router.backend_for(zone_ctx);
        let state_repo: Arc<dyn ZoneStateRepo> = router.state_repo_for(zone_ctx);

        let room_fut = backend.room_by_id(&zone_ctx.blueprint, room_id);
        let exits_fut = backend.room_exits(room_id);
        let objects_fut = backend.room_objects(room_id);
        let scripts_fut = backend.room_scripts(room_id);
        let room_kv_fut = backend.room_kv(room_id);
        let state_fut = state_repo.zone_room_state(zone_ctx, room_id, account_id);

        let (room, exits, objects, scripts, room_kv, zone_state) = try_join!(
            room_fut,
            exits_fut,
            objects_fut,
            scripts_fut,
            room_kv_fut,
            state_fut,
        )?;

        Ok(RoomView {
            room,
            objects,
            scripts,
            room_kv,
            exits,
            zone_state: Some(zone_state),
        })
    }
}