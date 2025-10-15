#![allow(unused)]

use std::sync::Arc;
use crate::models::blueprint::Blueprint;
use crate::db::repo::room::{BlueprintAndRoomKey, RoomRepo};
use crate::error::AppResult;
use crate::models::room::{BlueprintRoom, RoomExitRow, RoomKv, RoomObject, RoomScripts, RoomView};
use crate::models::types::{AccountId, BlueprintId, RoomId, ScriptSource};

pub struct BlueprintService {
    repo: Arc<dyn RoomRepo>,
}

impl BlueprintService {
    pub fn new(repo: Arc<dyn RoomRepo>) -> Self {
        Self { repo }
    }

    pub async fn get_by_key(&self, bp_key: &str) -> AppResult<Blueprint> {
        let blueprint = self.repo.blueprint_by_key(bp_key).await?;
        Ok(blueprint)
    }

    pub async fn room_by_id(&self, bp_id: BlueprintId, room_id: RoomId) -> AppResult<BlueprintRoom> {
        let bp_room = self.repo.room_by_id(bp_id, room_id).await?;
        Ok(bp_room)
    }

    pub async fn room_by_key(&self, key: BlueprintAndRoomKey) -> AppResult<BlueprintRoom> {
        let bp_room = self.repo.room_by_key(&key).await?;
        Ok(bp_room)
    }

    pub async fn room_exits(&self, _bp_id: BlueprintId, room_id: RoomId) -> AppResult<Vec<RoomExitRow>> {
        let exits = self.repo.room_exits(room_id).await?;
        Ok(exits)
    }

    pub async fn room_objects(&self, _bp_id: BlueprintId, room_id: RoomId) -> AppResult<Vec<RoomObject>> {
        let objects = self.repo.room_objects(room_id).await?;
        Ok(objects)
    }

    pub async fn room_scripts(&self, _bp_id: BlueprintId, room_id: RoomId) -> AppResult<RoomScripts> {
        let scripts = self.repo.room_scripts(room_id, ScriptSource::Live).await?;
        Ok(scripts)
    }

    pub async fn room_kv(&self, _bp_id: BlueprintId, room_id: RoomId) -> AppResult<RoomKv> {
        let kv_pairs = self.repo.room_kv(room_id).await?;
        Ok(kv_pairs)
    }

    /// Adds an exit from one room to another in a blueprint.
    pub async fn add_exit(&self, from_key: &BlueprintAndRoomKey, dir: &str, to_key: &BlueprintAndRoomKey) -> AppResult<bool> {
        let res = self.repo.add_exit(from_key, dir, to_key).await?;
        Ok(res)
    }

    /// Sets the entry room for a blueprint.
    pub async fn set_entry(&self, key: &BlueprintAndRoomKey) -> AppResult<bool> {
        let res = self.repo.set_entry(key).await?;
        Ok(res)
    }

    /// Locks or unlocks a room in a blueprint.
    pub async fn set_locked(&self, key: &BlueprintAndRoomKey, locked: bool) -> AppResult<bool> {
        let res = self.repo.set_locked(key, locked).await?;
        Ok(res)
    }

    /// Creates a new blueprint.
    pub async fn new_blueprint(&self, bp_key: &str, title: &str, account_id: AccountId) -> AppResult<bool> {
        let res = self.repo.insert_blueprint(bp_key, title, account_id).await?;
        Ok(res)
    }

    /// Creates a new room in a blueprint.
    pub async fn new_room(&self, key: &BlueprintAndRoomKey, title: &str, body: &str) -> AppResult<bool> {
        let res = self.repo.insert_room(key, title, body).await?;
        Ok(res)
    }

    /// Submits a blueprint for review.
    pub async fn submit(&self, bp: &str) -> AppResult<bool> {
        let res = self.repo.submit(bp).await?;
        Ok(res)
    }
}