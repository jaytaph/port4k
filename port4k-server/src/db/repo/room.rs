use crate::models::blueprint::Blueprint;
use crate::models::room::{BlueprintRoom, RoomExitRow, RoomKv, RoomObject, RoomScripts, RoomView, ZoneRoomState};
use crate::models::types::{RoomId, ScriptSource, ZoneId};
use crate::error::AppResult;

#[async_trait::async_trait]
pub trait RoomRepo: Send + Sync {
    async fn get_blueprint(&self, bp_key: &str) -> AppResult<Blueprint>;
    async fn get_blueprint_room(&self, room_id: RoomId) -> AppResult<BlueprintRoom>;
    async fn get_exits(&self, room_id: RoomId) -> AppResult<Vec<RoomExitRow>>;
    async fn get_objects_with_nouns(&self, room_id: RoomId) -> AppResult<Vec<RoomObject>>;
    async fn get_scripts(&self, room_id: RoomId, src: ScriptSource) -> AppResult<RoomScripts>;
    async fn get_room_kv(&self, room_id: RoomId) -> AppResult<RoomKv>;
    async fn get_zone_state(&self, zone_id: ZoneId, room_id: RoomId) -> AppResult<Option<ZoneRoomState>>;
    async fn get_view(&self, room_id: RoomId, zone_id: Option<ZoneId>, scripts: ScriptSource) -> AppResult<RoomView>;

    async fn set_entry(&self, bp_key: &str, room_key: &str) -> AppResult<bool>;
    async fn add_exit(&self, bp_key: &str, from_key: &str, dir: &str, to_key: &str) -> AppResult<bool>;
    async fn set_locked(&self, bp_key: &str, room_key: &str, locked: bool) -> AppResult<bool>;
    async fn insert_blueprint(&self, bp_key: &str, title: &str, owner: &str) -> AppResult<bool>;
    async fn insert_room(&self, bp_key: &str, room_key: &str, title: &str, body: &str) -> AppResult<bool>;
    async fn submit(&self, bp_key: &str) -> AppResult<bool>;
}