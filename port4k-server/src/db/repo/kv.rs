use crate::db::DbResult;
use crate::models::types::{AccountId, RoomId};

#[async_trait::async_trait]
pub trait KvRepo: Send + Sync {
    async fn room_kv_get_all(&self, room_id: RoomId) -> DbResult<serde_json::Map<String, serde_json::Value>>;
    async fn room_kv_get(&self, room_id: RoomId, obj_key: &str) -> DbResult<serde_json::Value>;
    async fn room_kv_set(&self, room_id: RoomId, obj_key: &str, value: serde_json::Value) -> DbResult<bool>;

    async fn player_kv_get(
        &self,
        room_id: RoomId,
        account_id: AccountId,
        obj_key: &str,
    ) -> DbResult<Option<serde_json::Value>>;
    async fn player_kv_set(
        &self,
        room_id: RoomId,
        account_id: AccountId,
        obj_key: &str,
        value: serde_json::Value,
    ) -> DbResult<bool>;
}
