use crate::db::DbResult;
use crate::models::realm::Realm;
use crate::models::room::Kv;
use crate::models::types::{AccountId, ExitId, ObjectId, RealmId, RoomId};
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait RealmRepo: Send + Sync {
    async fn get(&self, realm_id: RealmId) -> DbResult<Option<Realm>>;
    async fn get_by_key(&self, key: &str) -> DbResult<Option<Realm>>;
    async fn create(&self, realm: Realm) -> DbResult<Realm>;
    async fn find_by_owner(&self, owner_id: AccountId) -> DbResult<Vec<Realm>>;

    async fn room_kv(&self, realm_id: RealmId, room_id: RoomId) -> DbResult<Kv>;
    async fn obj_kv(&self, realm_id: RealmId, room_id: RoomId) -> DbResult<HashMap<String, Kv>>;

    async fn set_room_kv(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        key: &str,
        value: &serde_json::Value,
    ) -> DbResult<()>;

    async fn set_object_kv(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        key: &str,
        value: &serde_json::Value,
    ) -> DbResult<()>;

    async fn set_exit_locked(&self, realm_id: RealmId, room_id: RoomId, exit_id: ExitId, locked: bool) -> DbResult<()>;
}
