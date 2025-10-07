use crate::db::DbResult;
use crate::models::blueprint::Blueprint;
use crate::models::room::{BlueprintRoom, RoomExitRow, RoomKv, RoomObject, RoomScripts, RoomView, ZoneRoomState};
use crate::models::types::{AccountId, RoomId, ScriptSource, ZoneId};

#[async_trait::async_trait]
pub trait RoomRepo: Send + Sync {
    async fn get_blueprint(&self, bp_key: &str) -> DbResult<Blueprint>;
    async fn get_blueprint_room(&self, room_id: RoomId) -> DbResult<BlueprintRoom>;
    async fn get_exits(&self, room_id: RoomId) -> DbResult<Vec<RoomExitRow>>;
    async fn get_objects_with_nouns(&self, room_id: RoomId) -> DbResult<Vec<RoomObject>>;
    async fn get_scripts(&self, room_id: RoomId, src: ScriptSource) -> DbResult<RoomScripts>;
    async fn get_room_kv(&self, room_id: RoomId) -> DbResult<RoomKv>;
    async fn get_zone_state(&self, zone_id: ZoneId, room_id: RoomId) -> DbResult<Option<ZoneRoomState>>;
    async fn get_view(&self, room_id: RoomId, zone_id: Option<ZoneId>, scripts: ScriptSource) -> DbResult<RoomView>;

    async fn set_entry(&self, bp_key: &str, room_key: &str) -> DbResult<bool>;
    async fn add_exit(&self, bp_key: &str, from_key: &str, dir: &str, to_key: &str) -> DbResult<bool>;
    async fn set_locked(&self, bp_key: &str, room_key: &str, locked: bool) -> DbResult<bool>;
    async fn insert_blueprint(&self, bp_key: &str, title: &str, account_id: AccountId) -> DbResult<bool>;
    async fn insert_room(&self, bp_key: &str, room_key: &str, title: &str, body: &str) -> DbResult<bool>;
    async fn submit(&self, bp_key: &str) -> DbResult<bool>;
}