use crate::db::repo::{RealmRepo, UserRepo};
use crate::error::{AppResult, DomainError};
use crate::models::realm::{Realm, RealmKind};
use crate::models::types::{AccountId, BlueprintId, ObjectId, RealmId, RoomId};
use crate::services::realm::storage_db::DbStorage;
use crate::services::realm::storage_mem::MemoryStorage;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;

mod storage_db; // Persistent storage
mod storage_mem; // Ephemeral storage

#[allow(unused)]
#[async_trait]
pub trait StateStorage: Send + Sync {
    async fn update_realm_room_kv(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool>;
    async fn update_user_room_kv(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        account_id: AccountId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool>;

    async fn update_realm_object_kv(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool>;
    async fn update_user_object_kv(
        &self,
        realm_id: RealmId,
        account_id: AccountId,
        object_id: ObjectId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool>;

    async fn set_current_room(&self, realm_id: RealmId, account_id: AccountId, to_room: RoomId) -> AppResult<()>;
    async fn record_travel(&self, realm_id: RealmId, account_id: AccountId, from: RoomId, to: RoomId) -> AppResult<()>;
}

pub struct RealmService {
    realm_repo: Arc<dyn RealmRepo>,
    #[allow(unused)]
    db_storage: Arc<DbStorage>,
    #[allow(unused)]
    mem_storage: Arc<MemoryStorage>,
}

impl RealmService {
    pub fn new(realm_repo: Arc<dyn RealmRepo>, user_repo: Arc<dyn UserRepo>) -> Self {
        let db_storage = Arc::new(DbStorage::new(realm_repo.clone(), user_repo.clone()));
        let mem_storage = Arc::new(MemoryStorage::new());

        Self {
            realm_repo,
            db_storage,
            mem_storage,
        }
    }

    pub async fn get_by_id(&self, realm_id: RealmId) -> AppResult<Option<Realm>> {
        let realm = self.realm_repo.get(realm_id).await?;
        Ok(realm)
    }

    pub async fn get_by_key(&self, realm_key: &str) -> AppResult<Option<Realm>> {
        let realm = self.realm_repo.get_by_key(realm_key).await?;
        Ok(realm)
    }

    pub fn create_ephemeral_realm(&self, owner: AccountId, bp_id: BlueprintId, title: String) -> Realm {
        Realm {
            id: RealmId::new(),
            bp_id,
            title,
            kind: RealmKind::Test { owner },
            created_at: Utc::now(),
        }
    }

    pub async fn create_persistent_realm(
        &self,
        bp_id: BlueprintId,
        title: String,
        kind: RealmKind,
    ) -> AppResult<Realm> {
        if matches!(kind, RealmKind::Test { .. }) {
            return Err(DomainError::InvalidData(
                "Cannot create persistent realm of Test kind".into(),
            ));
        }

        let realm = Realm {
            id: RealmId::new(),
            bp_id,
            title,
            kind,
            created_at: Utc::now(),
        };

        let realm = self.realm_repo.create(realm).await?;
        Ok(realm)
    }

    pub async fn get_realm(&self, realm_id: RealmId) -> AppResult<Option<Realm>> {
        let realm = self.realm_repo.get(realm_id).await?;
        Ok(realm)
    }

    // pub async fn get_or_create_live_realm(&self, bp_id: BlueprintId) -> AppResult<Realm> {
    //     // Try to find an existing live realm for the blueprint
    //     // If not found, create a new persistent realm of Live kind
    //     unimplemented!()
    // }
}
