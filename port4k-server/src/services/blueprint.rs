#![allow(unused)]

use std::sync::Arc;
use crate::db::repo::room::RoomRepo;

pub struct BlueprintService {
    repo: Arc<dyn RoomRepo>,
}

impl BlueprintService {
    pub fn new(repo: Arc<dyn RoomRepo>) -> Self {
        Self { repo }
    }

    /// Adds an exit from one room to another in a blueprint.
    pub async fn add_exit(&self, bp: &str, from: &str, dir: &str, to: &str) -> anyhow::Result<bool> {
        self.repo.add_exit(bp, from, dir, to).await
    }

    /// Sets the entry room for a blueprint.
    pub async fn set_entry(&self, bp: &str, room: &str) -> anyhow::Result<bool> {
        self.repo.set_entry(bp, room).await
    }

    /// Locks or unlocks a room in a blueprint.
    pub async fn set_locked(&self, bp: &str, room: &str, locked: bool) -> anyhow::Result<bool> {
        self.repo.set_locked(bp, room, locked).await
    }

    /// Creates a new blueprint.
    pub async fn new_blueprint(&self, bp: &str, title: &str, owner: &str) -> anyhow::Result<bool> {
        self.repo.insert_blueprint(bp, title, owner).await
    }

    /// Creates a new room in a blueprint.
    pub async fn new_room(&self, bp: &str, room: &str, title: &str, body: &str) -> anyhow::Result<bool> {
        self.repo.insert_room(bp, room, title, body).await
    }

    /// Submits a blueprint for review.
    pub async fn submit(&self, bp: &str) -> anyhow::Result<bool> {
        self.repo.submit(bp).await
    }
}