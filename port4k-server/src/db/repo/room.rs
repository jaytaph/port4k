use crate::db::DbResult;
use crate::db::repo::BlueprintAndRoomKey;
use crate::models::blueprint::Blueprint;
use crate::models::room::{BlueprintExit, BlueprintObject, BlueprintRoom, Kv, RoomScripts};
use crate::models::types::{AccountId, BlueprintId, RoomId};

// Since room_id's are globally unique, we don't really need the bp_key here, but we do it
// anyway to ensure that the room belongs to the given blueprint.
#[async_trait::async_trait]
pub trait RoomRepo: Send + Sync {
    async fn blueprint_by_key(&self, bp_key: &str) -> DbResult<Blueprint>;

    async fn room_by_id(&self, bp_id: BlueprintId, room_id: RoomId) -> DbResult<BlueprintRoom>;
    async fn room_by_key(&self, key: &BlueprintAndRoomKey) -> DbResult<BlueprintRoom>;

    async fn room_exits(&self, room_id: RoomId) -> DbResult<Vec<BlueprintExit>>;
    async fn room_objects(&self, room_id: RoomId) -> DbResult<Vec<BlueprintObject>>;
    async fn room_scripts(&self, room_id: RoomId) -> DbResult<RoomScripts>;
    async fn room_kv(&self, room_id: RoomId) -> DbResult<Kv>;

    async fn set_entry(&self, key: &BlueprintAndRoomKey) -> DbResult<bool>;
    async fn add_exit(&self, from_key: &BlueprintAndRoomKey, dir: &str, to_key: &BlueprintAndRoomKey)
    -> DbResult<bool>;
    async fn set_locked(&self, key: &BlueprintAndRoomKey, locked: bool) -> DbResult<bool>;
    async fn insert_blueprint(&self, bp_key: &str, title: &str, account_id: AccountId) -> DbResult<bool>;
    async fn insert_room(&self, key: &BlueprintAndRoomKey, title: &str, body: &str) -> DbResult<bool>;
    async fn submit(&self, bp_key: &str) -> DbResult<bool>;
}
