use std::sync::Arc;
use crate::db::repo::kv::KvRepo;
use crate::models::types::{AccountId, RoomId};
use crate::services::ServiceResult;

pub struct RoomService {
    repo: Arc<dyn KvRepo>,
}

impl RoomService {
    pub fn new(repo: Arc<dyn KvRepo>) -> Self {
        Self { repo }
    }

    pub async fn room_kv_get(&self, room_id: RoomId, object_key: &str) -> ServiceResult<serde_json::Value> {
        let v = self.repo.room_kv_get(room_id, object_key).await?;
        Ok(v)
    }

    pub async fn room_kv_set(&self, room_id: RoomId, object_key: &str, v: &serde_json::Value) -> ServiceResult<()> {
        self.repo.room_kv_set(room_id, object_key, v.clone()).await?;
        Ok(())
    }

    pub async fn player_kv_get(&self, room_id: RoomId, account_id: AccountId, object_key: &str) -> ServiceResult<Option<serde_json::Value>> {
        let v = self.repo.player_kv_get(room_id, account_id, object_key).await?;
        Ok(v)
    }

    pub async fn player_kv_set(&self, room_id: RoomId, account_id: AccountId, object_key: &str, v: &serde_json::Value) -> ServiceResult<()> {
        self.repo.player_kv_set(room_id, account_id, object_key, v.clone()).await?;
        Ok(())
    }
}