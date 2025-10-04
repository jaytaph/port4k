use anyhow::Result;
use crate::db::models::room::{BlueprintRoom, RoomExitRow, RoomKv, RoomObject, RoomScripts, RoomView, ZoneRoomState};
use crate::db::types::{RoomId, ScriptSource, ZoneId};

#[async_trait::async_trait]
pub trait RoomRepo: Send + Sync {
    async fn get_blueprint_room(&self, room_id: RoomId) -> Result<BlueprintRoom>;
    async fn get_exits(&self, room_id: RoomId) -> Result<Vec<RoomExitRow>>;
    async fn get_objects_with_nouns(&self, room_id: RoomId) -> Result<Vec<RoomObject>>;
    async fn get_scripts(&self, room_id: RoomId, src: ScriptSource) -> Result<RoomScripts>;
    async fn get_room_kv(&self, room_id: RoomId) -> Result<RoomKv>;
    async fn get_zone_state(&self, zone_id: ZoneId, room_id: RoomId) -> Result<Option<ZoneRoomState>>;
    async fn get_view(&self, room_id: RoomId, zone_id: Option<ZoneId>, scripts: ScriptSource) -> Result<RoomView>;

    async fn set_entry(&self, bp_key: &str, room_key: &str) -> Result<bool>;
    async fn add_exit(&self, bp_key: &str, from_key: &str, dir: &str, to_key: &str) -> Result<bool>;
    async fn set_locked(&self, bp_key: &str, room_key: &str, locked: bool) -> Result<bool>;
    async fn insert_blueprint(&self, bp_key: &str, title: &str, owner: &str) -> Result<bool>;
    async fn insert_room(&self, bp_key: &str, room_key: &str, title: &str, body: &str) -> Result<bool>;
    async fn submit(&self, bp_key: &str) -> Result<bool>;
}