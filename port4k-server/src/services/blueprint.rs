#![allow(unused)]

use std::sync::Arc;
use crate::db::models::blueprint::Blueprint;
use crate::db::repo::room::RoomRepo;

pub struct BlueprintService {
    repo: Arc<dyn RoomRepo>,
}

impl BlueprintService {
    pub fn new(repo: Arc<dyn RoomRepo>) -> Self {
        Self { repo }
    }

    pub fn get_by_key(&self, bp_key: &str) -> AppResult<Blueprint> {
        self.repo.get_blueprint(bp_key)
    }

    /// Adds an exit from one room to another in a blueprint.
    pub async fn add_exit(&self, bp: &str, from: &str, dir: &str, to: &str) -> AppResult<bool> {
        self.repo.add_exit(bp, from, dir, to).await
    }

    /// Sets the entry room for a blueprint.
    pub async fn set_entry(&self, bp: &str, room: &str) -> AppResult<bool> {
        self.repo.set_entry(bp, room).await
    }

    /// Locks or unlocks a room in a blueprint.
    pub async fn set_locked(&self, bp: &str, room: &str, locked: bool) -> AppResult<bool> {
        self.repo.set_locked(bp, room, locked).await
    }

    /// Creates a new blueprint.
    pub async fn new_blueprint(&self, bp: &str, title: &str, owner: &str) -> AppResult<bool> {
        self.repo.insert_blueprint(bp, title, owner).await
    }

    /// Creates a new room in a blueprint.
    pub async fn new_room(&self, bp: &str, room: &str, title: &str, body: &str) -> AppResult<bool> {
        self.repo.insert_room(bp, room, title, body).await
    }

    /// Submits a blueprint for review.
    pub async fn submit(&self, bp: &str) -> AppResult<bool> {
        self.repo.submit(bp).await
    }
}