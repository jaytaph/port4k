use std::collections::HashMap;
use crate::db::DbResult;
use crate::models::room::Kv;
use crate::models::types::{AccountId, RoomId, ZoneId};

#[async_trait::async_trait]
pub trait UserRepo: Send + Sync {
    async fn room_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId) -> DbResult<Kv>;
    async fn obj_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId) -> DbResult<HashMap<String, Kv>>;
}
