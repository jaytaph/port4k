use crate::db::repo::kv::KvRepo;
use crate::db::repo::room::RoomRepo;
use crate::error::{AppResult, DomainError};
use crate::models::room::RoomView;
use crate::models::types::{AccountId, RoomId, ScriptSource};
use crate::models::zone::{ZoneContext, ZoneRouter, ZoneState};
use std::sync::Arc;
use tokio::try_join;

pub struct RoomService {
    kv_repo: Arc<dyn KvRepo>,
    room_repo: Arc<dyn RoomRepo>,
}

impl RoomService {
    pub fn new(kv_repo: Arc<dyn KvRepo>, room_repo: Arc<dyn RoomRepo>) -> Self {
        Self { kv_repo, room_repo }
    }

    pub async fn room_kv_get_all(&self, room_id: RoomId) -> AppResult<serde_json::Map<String, serde_json::Value>> {
        let kv_pairs = self.kv_repo.room_kv_get_all(room_id).await?;
        Ok(kv_pairs)
    }

    /// Get room key-value data
    pub async fn room_kv_get(&self, room_id: RoomId, object_key: &str) -> AppResult<serde_json::Value> {
        let v = self.kv_repo.room_kv_get(room_id, object_key).await?;
        Ok(v)
    }

    /// Set room key-value data
    pub async fn room_kv_set(&self, room_id: RoomId, object_key: &str, v: &serde_json::Value) -> AppResult<()> {
        self.kv_repo.room_kv_set(room_id, object_key, v.clone()).await?;
        Ok(())
    }

    /// Get player key-value data for specific room
    pub async fn player_kv_get(
        &self,
        room_id: RoomId,
        account_id: AccountId,
        object_key: &str,
    ) -> AppResult<Option<serde_json::Value>> {
        let v = self.kv_repo.player_kv_get(room_id, account_id, object_key).await?;
        Ok(v)
    }

    /// Set player key-value data for specific room
    pub async fn player_kv_set(
        &self,
        room_id: RoomId,
        account_id: AccountId,
        object_key: &str,
        v: &serde_json::Value,
    ) -> AppResult<()> {
        self.kv_repo
            .player_kv_set(room_id, account_id, object_key, v.clone())
            .await?;
        Ok(())
    }

    /// Creates a RoomView for the given cursor in the given zone context
    pub async fn build_room_view(
        &self,
        router: Arc<ZoneRouter>,
        zone_ctx: &ZoneContext,
        account_id: AccountId,
        room_id: RoomId,
    ) -> AppResult<RoomView> {
        let zone_state: Arc<dyn ZoneState> = router.state_for(zone_ctx);

        let room_fut = async {
            self.room_repo
                .room_by_id(zone_ctx.blueprint.id, room_id)
                .await
                .map_err(DomainError::from)
        };
        let exits_fut = async { self.room_repo.room_exits(room_id).await.map_err(DomainError::from) };
        let objects_fut = async { self.room_repo.room_objects(room_id).await.map_err(DomainError::from) };
        let scripts_fut = async {
            self.room_repo
                .room_scripts(room_id, ScriptSource::Live)
                .await
                .map_err(DomainError::from)
        };
        let kv_fut = async { self.room_repo.room_kv(room_id).await.map_err(DomainError::from) };
        let state_fut = async { zone_state.zone_room_state(&zone_ctx, room_id, account_id).await };

        let (room, exits, objects, scripts, room_kv, zone_state) =
            try_join!(room_fut, exits_fut, objects_fut, scripts_fut, kv_fut, state_fut)?;

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
