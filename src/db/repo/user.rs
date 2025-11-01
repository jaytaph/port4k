use crate::db::DbResult;
use crate::models::room::Kv;
use crate::models::types::{AccountId, ExitId, ObjectId, RealmId, RoomId};
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait UserRepo: Send + Sync {
    /// Gets all KV for a room for a specific user
    async fn room_kv(&self, realm_id: RealmId, room_id: RoomId, account_id: AccountId) -> DbResult<Kv>;
    /// Gets all object KVs for a specific user in a room
    async fn obj_kv(&self, realm_id: RealmId, room_id: RoomId, account_id: AccountId) -> DbResult<HashMap<String, Kv>>;

    /// Stores the current realm/room/account location
    async fn set_current_room(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        room_id: RoomId,
    ) -> DbResult<()>;

    /// Increase item in the room KV for a specific user
    async fn inc_room_kv(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        account_id: AccountId,
        key: &str,
        inc_by: i64,
    ) -> DbResult<i64>;

    /// Set item in the room KV for a specific user
    async fn set_room_kv(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        account_id: AccountId,
        key: &str,
        value: &serde_json::Value,
    ) -> DbResult<()>;

    /// Set item in the object KV for a specific user
    async fn set_object_kv(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        object_id: ObjectId,
        key: &str,
        value: &serde_json::Value,
    ) -> DbResult<()>;

    /// Set exit locked state for a specific user
    async fn set_exit_locked(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        account_id: AccountId,
        exit_id: ExitId,
        locked: bool,
    ) -> DbResult<()>;

    /// Get exit locked state for a specific user
    async fn is_exit_locked(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        account_id: AccountId,
        exit_id: ExitId,
    ) -> DbResult<bool>;
}
