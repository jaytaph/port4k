use crate::db::repo::{RealmRepo, UserRepo};
use crate::error::AppResult;
use crate::models::types::{AccountId, ObjectId, RealmId, RoomId};
use crate::services::realm::StateStorage;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct DbStorage {
    realm_repo: Arc<dyn RealmRepo>,
    user_repo: Arc<dyn UserRepo>,
}

impl DbStorage {
    pub fn new(realm_repo: Arc<dyn RealmRepo>, user_repo: Arc<dyn UserRepo>) -> Self {
        Self { realm_repo, user_repo }
    }
}

#[async_trait]
impl StateStorage for DbStorage {
    async fn update_realm_room_kv(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool> {
        // KV for the room at the realm level (shared)
        self.realm_repo.set_room_kv(realm_id, room_id, key, value).await?;
        Ok(true)
    }

    async fn update_user_room_kv(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        account_id: AccountId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool> {
        // KV for the room at the player level (private)
        self.user_repo
            .set_room_kv(realm_id, room_id, account_id, key, value)
            .await?;

        Ok(true)
    }

    async fn update_realm_object_kv(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool> {
        self.realm_repo.set_object_kv(realm_id, object_id, key, value).await?;

        Ok(true)
    }

    async fn update_user_object_kv(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        object_id: ObjectId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool> {
        self.user_repo
            .set_object_kv(realm_id, account_id, object_id, key, value)
            .await?;

        Ok(true)
    }

    async fn set_current_room(&self, realm_id: RealmId, account_id: AccountId, room_id: RoomId) -> AppResult<()> {
        self.user_repo.set_current_room(realm_id, account_id, room_id).await;
        Ok(())
    }

    async fn record_travel(
        &self,
        _realm_id: RealmId,
        _account_id: AccountId,
        _from: RoomId,
        _to: RoomId,
    ) -> AppResult<()> {
        // TODO: Implement travel history
        Ok(())
    }
}
