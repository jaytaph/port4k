#![allow(unused)]

use std::sync::Arc;
use crate::models::blueprint::Blueprint;
use crate::db::repo::room::RoomRepo;
use crate::error::AppResult;
use crate::models::room::RoomView;
use crate::models::types::AccountId;

pub struct BlueprintService {
    repo: Arc<dyn RoomRepo>,
}

impl BlueprintService {
    pub fn new(repo: Arc<dyn RoomRepo>) -> Self {
        Self { repo }
    }

    pub async fn get_by_key(&self, bp_key: &str) -> AppResult<Blueprint> {
        let blueprint = self.repo.get_blueprint(bp_key).await?;
        Ok(blueprint)
    }

    /// Adds an exit from one room to another in a blueprint.
    pub async fn add_exit(&self, bp: &str, from: &str, dir: &str, to: &str) -> AppResult<bool> {
        let res = self.repo.add_exit(bp, from, dir, to).await?;
        Ok(res)
    }

    /// Sets the entry room for a blueprint.
    pub async fn set_entry(&self, bp: &str, room: &str) -> AppResult<bool> {
        let res = self.repo.set_entry(bp, room).await?;
        Ok(res)
    }

    /// Locks or unlocks a room in a blueprint.
    pub async fn set_locked(&self, bp: &str, room: &str, locked: bool) -> AppResult<bool> {
        let res = self.repo.set_locked(bp, room, locked).await?;
        Ok(res)
    }

    /// Creates a new blueprint.
    pub async fn new_blueprint(&self, bp: &str, title: &str, account_id: AccountId) -> AppResult<bool> {
        let res = self.repo.insert_blueprint(bp, title, account_id).await?;
        Ok(res)
    }

    /// Creates a new room in a blueprint.
    pub async fn new_room(&self, bp: &str, room: &str, title: &str, body: &str) -> AppResult<bool> {
        let res = self.repo.insert_room(bp, room, title, body).await?;
        Ok(res)
    }

    /// Submits a blueprint for review.
    pub async fn submit(&self, bp: &str) -> AppResult<bool> {
        let res = self.repo.submit(bp).await?;
        Ok(res)
    }
}