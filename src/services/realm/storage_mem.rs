use crate::error::AppResult;
use crate::models::types::{AccountId, ObjectId, RealmId, RoomId};
use crate::services::realm::StateStorage;
use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[allow(unused)]
pub struct MemoryStorage {
    realms: DashMap<RealmId, Arc<MemRealm>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self { realms: DashMap::new() }
    }

    #[allow(unused)]
    fn realm(&self, id: RealmId) -> Arc<MemRealm> {
        self.realms
            .entry(id)
            .or_insert_with(|| Arc::new(MemRealm::default()))
            .clone()
    }
}

#[allow(unused)]
#[derive(Default)]
struct MemRealm {
    realm_room_kv: DashMap<RoomId, HashMap<String, Value>>,
    user_room_kv: DashMap<(RoomId, AccountId), HashMap<String, Value>>,
    realm_object_kv: DashMap<ObjectId, HashMap<String, Value>>,
    user_object_kv: DashMap<(ObjectId, AccountId), HashMap<String, Value>>,
    current_room: DashMap<AccountId, RoomId>,
}

#[async_trait]
impl StateStorage for MemoryStorage {
    async fn update_realm_room_kv(
        &self,
        realm_id: RealmId,
        room_id: RoomId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool> {
        let realm = self.realm(realm_id);
        let mut inner = realm.realm_room_kv.entry(room_id).or_default();
        inner.insert(key.to_string(), value.clone());
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
        let realm = self.realm(realm_id);
        let mut inner = realm.user_room_kv.entry((room_id, account_id)).or_default();
        inner.insert(key.to_string(), value.clone());
        Ok(true)
    }

    async fn update_realm_object_kv(
        &self,
        realm_id: RealmId,
        object_id: ObjectId,
        key: &str,
        value: &Value,
    ) -> AppResult<bool> {
        let realm = self.realm(realm_id);
        let mut inner = realm.realm_object_kv.entry(object_id).or_default();
        inner.insert(key.to_string(), value.clone());
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
        let realm = self.realm(realm_id);
        let mut inner = realm.user_object_kv.entry((object_id, account_id)).or_default();
        inner.insert(key.to_string(), value.clone());
        Ok(true)
    }

    async fn set_current_room(&self, realm_id: RealmId, account_id: AccountId, room_id: RoomId) -> AppResult<()> {
        let realm = self.realm(realm_id);
        realm.current_room.insert(account_id, room_id);
        Ok(())
    }

    async fn record_travel(
        &self,
        _realm_id: RealmId,
        _account_id: AccountId,
        _from: RoomId,
        _to: RoomId,
    ) -> AppResult<()> {
        Ok(())
    }
}
