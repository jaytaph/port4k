use crate::db::DbResult;
use crate::models::blueprint::Blueprint;
use crate::models::room::{BlueprintRoom, RoomExitRow, RoomKv, RoomObject, RoomScripts};
use crate::models::types::{AccountId, RoomId, ScriptSource};

// Since room_id's are globally unique, we don't really need the bp_key here, but we do it
// anyway to ensure that the room belongs to the given blueprint.
#[async_trait::async_trait]
pub trait RoomRepo: Send + Sync {
    async fn blueprint_by_key(&self, bp_key: &str) -> DbResult<Blueprint>;
    async fn room(&self, room_id: RoomId) -> DbResult<BlueprintRoom>;
    async fn room_exits(&self, room_id: RoomId) -> DbResult<Vec<RoomExitRow>>;
    async fn room_objects(&self, room_id: RoomId) -> DbResult<Vec<RoomObject>>;
    async fn room_scripts(&self, room_id: RoomId, src: ScriptSource) -> DbResult<RoomScripts>;
    async fn room_kv(&self, room_id: RoomId) -> DbResult<RoomKv>;

    async fn set_entry(&self, bp_key: &str, room_key: &str) -> DbResult<bool>;
    async fn add_exit(&self, bp_key: &str, from_key: &str, dir: &str, to_key: &str) -> DbResult<bool>;
    async fn set_locked(&self, bp_key: &str, room_key: &str, locked: bool) -> DbResult<bool>;
    async fn insert_blueprint(&self, bp_key: &str, title: &str, account_id: AccountId) -> DbResult<bool>;
    async fn insert_room(&self, bp_key: &str, room_key: &str, title: &str, body: &str) -> DbResult<bool>;
    async fn submit(&self, bp_key: &str) -> DbResult<bool>;
}