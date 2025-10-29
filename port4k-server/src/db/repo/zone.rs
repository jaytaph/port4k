use crate::db::DbResult;
use crate::models::room::Kv;
use crate::models::types::{ExitId, ObjectId, RoomId, ZoneId};
use crate::models::zone::Zone;
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait ZoneRepo: Send + Sync {
    async fn get_by_key(&self, zone_key: &str) -> DbResult<Option<Zone>>;

    async fn room_kv(&self, zone_id: ZoneId, room_id: RoomId) -> DbResult<Kv>;
    async fn obj_kv(&self, zone_id: ZoneId, room_id: RoomId) -> DbResult<HashMap<String, Kv>>;

    async fn set_object_kv(
        &self,
        zone_id: ZoneId,
        object_id: ObjectId,
        key: &str,
        value: &serde_json::Value,
    ) -> DbResult<()>;

    async fn set_exit_locked(&self, zone_id: ZoneId, room_id: RoomId, exit_id: ExitId, locked: bool) -> DbResult<()>;
}
