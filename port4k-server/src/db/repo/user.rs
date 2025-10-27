use crate::db::DbResult;
use crate::models::room::Kv;
use crate::models::types::{AccountId, ExitId, RoomId, ZoneId};
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait UserRepo: Send + Sync {
    async fn room_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId) -> DbResult<Kv>;
    async fn obj_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId) -> DbResult<HashMap<String, Kv>>;

    async fn inc_room_kv(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        key: &str,
        inc_by: i64,
    ) -> DbResult<i64>;
    async fn set_room_kv(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        key: &str,
        value: &serde_json::Value,
    ) -> DbResult<()>;
    async fn set_exit_locked(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        exit_id: ExitId,
        locked: bool,
    ) -> DbResult<()>;
}
