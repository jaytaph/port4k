use crate::db::error::DbError;
use crate::db::DbResult;
use crate::models::types::{AccountId, RealmId, BlueprintId, RoomId};
use chrono::{DateTime, Utc};
use tokio_postgres::Row;

/// Type of realm defines what is allowed and how it is persisted.
#[derive(Clone, Debug)]
pub enum RealmKind {
    /// Production realm
    Live,
    /// Draft realm for building and testing
    Draft,
    /// Temporary test realm for a specific user
    Test { owner: AccountId },
}

impl RealmKind {
    pub fn to_string(&self) -> String {
        match self {
            RealmKind::Live => "live".to_string(),
            RealmKind::Draft => "draft".to_string(),
            RealmKind::Test { .. } => "test".to_string(),
        }
    }
}

/// How the realm is persisted.
#[derive(Clone, Debug)]
pub enum Persistence {
    Ephemeral,  // Save to memory only
    Persistent, // Save to database / disk
}

impl Persistence {
    #[inline]
    pub fn is_ephemeral(&self) -> bool {
        matches!(self, Persistence::Ephemeral)
    }
    #[inline]
    pub fn is_persistent(&self) -> bool {
        matches!(self, Persistence::Persistent)
    }
}

// /// Total realm policy (for now, just persistence)
// #[derive(Clone, Debug)]
// pub struct RealmPolicy {
//     pub persistence: Persistence,
// }

// impl RealmPolicy {
//     pub fn for_kind(kind: &RealmKind) -> Self {
//         match kind {
//             RealmKind::Live => RealmPolicy {
//                 persistence: Persistence::Persistent,
//             },
//             RealmKind::Draft => RealmPolicy {
//                 persistence: Persistence::Persistent,
//             },
//             RealmKind::Test { .. } => RealmPolicy {
//                 persistence: Persistence::Ephemeral,
//             },
//         }
//     }
// }
//
// /// Realm context
// #[derive(Clone, Debug)]
// pub struct RealmContext {
//     /// Which realm are we in
//     pub realm: Arc<Realm>,
//     /// Kind of realm
//     pub kind: RealmKind,
//     /// Policy of the realm
//     pub policy: RealmPolicy,
//     /// Blueprint on which this realm is based
//     pub blueprint: Arc<Blueprint>,
// }
//
// impl RealmContext {
//     pub fn new(realm: Arc<Realm>, blueprint: Arc<Blueprint>) -> Self {
//         let kind = realm.kind.clone();
//         let policy = RealmPolicy::for_kind(&kind);
//         Self {
//             realm,
//             kind,
//             policy,
//             blueprint,
//         }
//     }
//
//     pub fn ephemeral(owner: AccountId, blueprint: Arc<Blueprint>) -> Self {
//         let realm_id = RealmId::new();
//         let realm = Arc::new(Realm {
//             id: realm_id,
//             bp_id: blueprint.id,
//             title: "Ephemeral Realm".into(),
//             kind: RealmKind::Test { owner },
//             created_at: Utc::now(),
//         });
//
//         Self::new(realm, blueprint)
//     }
// }

// Realm model as stored in DB
#[derive(Debug, Clone)]
pub struct Realm {
    /// Realm id
    pub id: RealmId,
    /// Blueprint ID
    pub bp_id: BlueprintId,
    /// Title of the realm
    pub title: String,
    /// Kind of realm
    pub kind: RealmKind,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl Realm {
    pub fn is_ephemeral(&self) -> bool {
        matches!(self.kind, RealmKind::Test { .. })
    }

    pub fn is_persistent(&self) -> bool {
        !self.is_ephemeral()
    }

    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let kind_s: &str = row.try_get("kind")?;
        let kind = match kind_s {
            "live" => RealmKind::Live,
            "draft" => RealmKind::Draft,
            _ => return Err(DbError::Decode("invalid realm.kind".into())),
        };

            Ok(Realm {
                id: row.try_get("id")?,
                bp_id: row.try_get("bp_id")?,
                title: row.try_get("title")?,
                kind,
                created_at: row.try_get("created_at")?,
            })
    }
}


//
// /// Router that defines how to access realm backends based on realm policy
// pub struct RealmRouter {
//     db: Arc<DbBackend>,
//     mem: Arc<MemoryBackend>,
// }
//
// impl RealmRouter {
//     pub fn new(db: Arc<DbBackend>, mem: Arc<MemoryBackend>) -> Self {
//         Self { db, mem }
//     }
//
//     pub fn storage_for(&self, realm_ctx: &RealmContext) -> Arc<dyn StateStorage> {
//         match realm_ctx.policy.persistence {
//             Persistence::Ephemeral => self.mem.clone(),
//             Persistence::Persistent => self.db.clone(),
//         }
//     }
// }
// // -----------------------------------------------------------------------------------------------
// pub struct DbBackend {
//     db: Arc<Db>,
// }
//
// impl DbBackend {
//     pub fn new(db: Arc<Db>) -> Self {
//         Self { db }
//     }
// }
//
// #[async_trait]
// impl StateStorage for DbBackend {
//     async fn update_realm_room_kv(&self, realm_id: RealmId, room_id: RoomId, key: &str, value: Value) -> AppResult<bool> {
//         let client = self.db.get_client().await?;
//
//         let _ = client
//             .execute(
//                 "INSERT INTO realm_room_kv (realm_id, room_id, key, value)
//                 VALUES ($1, $2, $3, $4)
//                 ON CONFLICT (realm_id, room_id, key)
//                 DO UPDATE SET value = EXCLUDED.value",
//                 &[&realm_id, &room_id, &key, &value],
//             )
//             .await
//             .map_err(DbError::from)?;
//
//         Ok(true)
//     }
//
//     async fn update_user_room_kv(
//         &self,
//         realm_id: RealmId,
//         room_id: RoomId,
//         account_id: AccountId,
//         key: &str,
//         value: Value,
//     ) -> AppResult<bool> {
//         let client = self.db.get_client().await?;
//
//         let _ = client
//             .execute(
//                 "INSERT INTO user_room_kv (realm_id, room_id, account_id, key, value)
//                 VALUES ($1, $2, $3, $4, $5)
//                 ON CONFLICT (realm_id, room_id, account_id, key)
//                 DO UPDATE SET value = EXCLUDED.value",
//                 &[&realm_id, &room_id, &account_id, &key, &value],
//             )
//             .await
//             .map_err(DbError::from)?;
//
//         Ok(true)
//     }
//
//     async fn update_realm_object_kv(
//         &self,
//         realm_id: RealmId,
//         object_id: ObjectId,
//         key: &str,
//         value: Value,
//     ) -> AppResult<bool> {
//         let client = self.db.get_client().await?;
//
//         let _ = client
//             .execute(
//                 "INSERT INTO realm_object_kv (realm_id, object_id, key, value)
//                 VALUES ($1, $2, $3, $4)
//                 ON CONFLICT (realm_id, object_id, key)
//                 DO UPDATE SET value = EXCLUDED.value",
//                 &[&realm_id, &object_id, &key, &value],
//             )
//             .await
//             .map_err(DbError::from)?;
//
//         Ok(true)
//     }
//
//     async fn update_user_object_kv(
//         &self,
//         realm_id: RealmId,
//         account_id: AccountId,
//         object_id: ObjectId,
//         key: &str,
//         value: Value,
//     ) -> AppResult<bool> {
//         let client = self.db.get_client().await?;
//
//         let _ = client
//             .execute(
//                 "INSERT INTO user_object_kv (realm_id, account_id, object_id, key, value)
//                 VALUES ($1, $2, $3, $4, $5)
//                 ON CONFLICT (realm_id, account_id, object_id, key)
//                 DO UPDATE SET value = EXCLUDED.value",
//                 &[&realm_id, &account_id, &object_id, &key, &value],
//             )
//             .await
//             .unwrap();
//
//         Ok(true)
//     }
//
//     async fn set_current_room(&self, realm_id: RealmId, account_id: AccountId, to_room: RoomId) -> AppResult<()> {
//         let client = self.db.get_client().await?;
//
//         let _ = client
//             .execute(
//                 "UPDATE characters SET room_id = $1
//             WHERE realm_id = $2 AND account_id = $3",
//                 &[&to_room, &realm_id, &account_id],
//             )
//             .await
//             .map_err(DbError::from)?;
//
//         Ok(())
//     }
//
//     async fn record_travel(
//         &self,
//         _realm_id: RealmId,
//         _account_id: AccountId,
//         _from: RoomId,
//         _to: RoomId,
//     ) -> AppResult<()> {
//         todo!()
//     }
// }
//
// // -----------------------------------------------------------------------------------------------
// pub struct MemoryBackend {
//     realms: DashMap<RealmId, Arc<MemRealm>>,
// }
//
// impl Default for MemoryBackend {
//     fn default() -> Self {
//         Self::new()
//     }
// }
//
// impl MemoryBackend {
//     pub fn new() -> Self {
//         Self { realms: DashMap::new() }
//     }
//
//     fn realm(&self, id: RealmId) -> Arc<MemRealm> {
//         self.realms
//             .entry(id)
//             .or_insert_with(|| Arc::new(MemRealm::default()))
//             .clone()
//     }
// }
//
// // enum StringOrVec {
// //     Str(String),
// //     Vec(Vec<String>),
// // }
//
// // impl TryFrom<Value> for StringOrVec {
// //     type Error = DomainError;
// //
// //     fn try_from(v: Value) -> Result<Self, Self::Error> {
// //         match v {
// //             Value::String(s) => Ok(StringOrVec::Str(s)),
// //             Value::Array(arr) => {
// //                 let mut out = Vec::with_capacity(arr.len());
// //                 for item in arr {
// //                     match item {
// //                         Value::String(s) => out.push(s),
// //                         _ => {},
// //                     }
// //                 }
// //                 Ok(StringOrVec::Vec(out))
// //             }
// //             _ => Ok(StringOrVec::Str("".into()))
// //         }
// //     }
// // }
//
// #[derive(Default)]
// struct MemRealm {
//     realm_room_kv: DashMap<RoomId, HashMap<String, String>>,
//     user_room_kv: DashMap<(RoomId, AccountId), HashMap<String, String>>,
//
//     realm_object_kv: DashMap<ObjectId, HashMap<String, String>>,
//     user_object_kv: DashMap<(ObjectId, AccountId), HashMap<String, String>>,
//
//     // room_qty: DashMap<(RoomId, ObjectId), i32>,
//     // coins: DashMap<AccountId, i32>,
//     // items: DashMap<(AccountId, ObjectId), i32>,
//     // health: DashMap<AccountId, i32>,
//     // xp: DashMap<AccountId, i32>,
//     current_room: DashMap<AccountId, RoomId>,
// }
//
// #[async_trait]
// impl StateStorage for MemoryBackend {
//     async fn update_realm_room_kv(&self, realm_id: RealmId, room_id: RoomId, key: &str, value: Value) -> AppResult<bool> {
//         let realm = self.realm(realm_id);
//
//         let mut inner = realm.realm_room_kv.entry(room_id).or_insert_with(HashMap::new);
//         inner.insert(key.to_string(), serde_to_str(value));
//
//         Ok(true)
//     }
//
//     async fn update_user_room_kv(
//         &self,
//         realm_id: RealmId,
//         room_id: RoomId,
//         account_id: AccountId,
//         key: &str,
//         value: Value,
//     ) -> AppResult<bool> {
//         let realm = self.realm(realm_id);
//
//         let mut inner = realm
//             .user_room_kv
//             .entry((room_id, account_id))
//             .or_insert_with(HashMap::new);
//         inner.insert(key.to_string(), serde_to_str(value));
//
//         Ok(true)
//     }
//
//     async fn update_realm_object_kv(
//         &self,
//         realm_id: RealmId,
//         object_id: ObjectId,
//         key: &str,
//         value: Value,
//     ) -> AppResult<bool> {
//         let realm = self.realm(realm_id);
//
//         let mut inner = realm.realm_object_kv.entry(object_id).or_insert_with(HashMap::new);
//         inner.insert(key.to_string(), serde_to_str(value));
//
//         Ok(true)
//     }
//
//     async fn update_user_object_kv(
//         &self,
//         realm_id: RealmId,
//         account_id: AccountId,
//         object_id: ObjectId,
//         key: &str,
//         value: Value,
//     ) -> AppResult<bool> {
//         let realm = self.realm(realm_id);
//
//         let mut inner = realm
//             .user_object_kv
//             .entry((object_id, account_id))
//             .or_insert_with(HashMap::new);
//         inner.insert(key.to_string(), serde_to_str(value));
//
//         Ok(true)
//     }
//
//     async fn set_current_room(&self, realm_id: RealmId, account_id: AccountId, to_room: RoomId) -> AppResult<()> {
//         let realm = self.realm(realm_id);
//         realm.current_room.insert(account_id, to_room);
//
//         Ok(())
//     }
//
//     async fn record_travel(
//         &self,
//         _realm_id: RealmId,
//         _account_id: AccountId,
//         _from: RoomId,
//         _to: RoomId,
//     ) -> AppResult<()> {
//         // @TODO: We don't track travel history in memory for now
//         Ok(())
//     }
// }
