use crate::db::DbResult;
use crate::models::blueprint::Blueprint;
use crate::models::room::{BlueprintExit, BlueprintObject, BlueprintRoom, Kv, RoomScripts};
use crate::models::types::{AccountId, BlueprintId, RoomId, ScriptSource};

/// Even though room_ids are globally unique, we still use a combination of
/// blueprint key and room key to identify a room sometimes. So we have a combined key
/// with the unique identifiers for both the blueprint and the room. This combination is also
/// globally unique.
/// The reason we do this is that when creating or editing a blueprint, the rooms
/// are identified by their blueprint key and room key, not by their room_id.
/// The room_id is only assigned when the room is created in the database.
/// This allows us to refer to rooms in a blueprint before they are created in the database.
#[derive(Debug, Clone)]
pub struct BlueprintAndRoomKey {
    pub bp_key: String,
    pub room_key: String,
}

impl BlueprintAndRoomKey {
    pub fn new(bp_key: &str, room_key: &str) -> Self {
        Self {
            bp_key: bp_key.to_string(),
            room_key: room_key.to_string(),
        }
    }
}

// Since room_id's are globally unique, we don't really need the bp_key here, but we do it
// anyway to ensure that the room belongs to the given blueprint.
#[async_trait::async_trait]
pub trait RoomRepo: Send + Sync {
    async fn blueprint_by_key(&self, bp_key: &str) -> DbResult<Blueprint>;

    async fn room_by_id(&self, bp_id: BlueprintId, room_id: RoomId) -> DbResult<BlueprintRoom>;
    async fn room_by_key(&self, key: &BlueprintAndRoomKey) -> DbResult<BlueprintRoom>;

    async fn room_exits(&self, room_id: RoomId) -> DbResult<Vec<BlueprintExit>>;
    async fn room_objects(&self, room_id: RoomId) -> DbResult<Vec<BlueprintObject>>;
    async fn room_scripts(&self, room_id: RoomId, src: ScriptSource) -> DbResult<RoomScripts>;
    async fn room_kv(&self, room_id: RoomId) -> DbResult<Kv>;

    async fn set_entry(&self, key: &BlueprintAndRoomKey) -> DbResult<bool>;
    async fn add_exit(&self, from_key: &BlueprintAndRoomKey, dir: &str, to_key: &BlueprintAndRoomKey)
    -> DbResult<bool>;
    async fn set_locked(&self, key: &BlueprintAndRoomKey, locked: bool) -> DbResult<bool>;
    async fn insert_blueprint(&self, bp_key: &str, title: &str, account_id: AccountId) -> DbResult<bool>;
    async fn insert_room(&self, key: &BlueprintAndRoomKey, title: &str, body: &str) -> DbResult<bool>;
    async fn submit(&self, bp_key: &str) -> DbResult<bool>;
}
