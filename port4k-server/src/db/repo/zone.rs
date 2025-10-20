use std::collections::HashMap;
use crate::db::DbResult;
use crate::models::room::Kv;
use crate::models::types::{RoomId, ZoneId};
use crate::models::zone::Zone;

#[async_trait::async_trait]
pub trait ZoneRepo: Send + Sync {
    async fn get_by_key(&self, zone_key: &str) -> DbResult<Option<Zone>>;

    async fn room_kv(&self, zone_id: ZoneId, room_id: RoomId) -> DbResult<Kv>;
    async fn obj_kv(&self, zone_id: ZoneId, room_id: RoomId) -> DbResult<HashMap<String, Kv>>;
}
