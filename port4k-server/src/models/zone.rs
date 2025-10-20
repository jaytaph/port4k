use crate::db::error::DbError;
use crate::db::{Db, DbResult};
use crate::error::AppResult;
use crate::models::blueprint::Blueprint;
use crate::models::types::{AccountId, ObjectId, RoomId, ZoneId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::Value;
use tokio_postgres::Row;
use crate::util::serde::serde_to_str;

/// Type of zone defines what is allowed and how it is persisted.
#[derive(Clone, Debug)]
pub enum ZoneKind {
    Live,
    Draft,
    Test { owner: AccountId },
}

/// How the zone is persisted.
#[derive(Clone, Debug)]
pub enum Persistence {
    Ephemeral,      // Save to memory only
    Persistent,     // Save to database / disk
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

/// Total zone policy (for now, just persistence)
#[derive(Clone, Debug)]
pub struct ZonePolicy {
    pub persistence: Persistence,
}

impl ZonePolicy {
    pub fn for_kind(kind: &ZoneKind) -> Self {
        match kind {
            ZoneKind::Live => ZonePolicy {
                persistence: Persistence::Persistent,
            },
            ZoneKind::Draft => ZonePolicy {
                persistence: Persistence::Persistent,
            },
            ZoneKind::Test { .. } => ZonePolicy {
                persistence: Persistence::Ephemeral,
            },
        }
    }
}

/// Zone context
#[derive(Clone, Debug)]
pub struct ZoneContext {
    /// Which zone are we in
    pub zone: Arc<Zone>,
    /// Kind of zone
    pub kind: ZoneKind,
    /// Policy of the zone
    pub policy: ZonePolicy,
    /// Blueprint on which this zone is based
    pub blueprint: Arc<Blueprint>,
}

impl ZoneContext {
    pub fn new(zone: Arc<Zone>, blueprint: Arc<Blueprint>) -> Self {
        let kind = zone.kind.clone();
        let policy = ZonePolicy::for_kind(&kind);
        Self {
            zone,
            kind,
            policy,
            blueprint,
        }
    }

    pub fn ephemeral(owner: AccountId, blueprint: Arc<Blueprint>) -> Self {
        let zone_id = ZoneId::new();
        let zone = Arc::new(Zone {
            id: zone_id,
            key: format!("ephemeral-{}", zone_id),
            title: "Ephemeral Zone".into(),
            kind: ZoneKind::Test { owner },
            created_at: Utc::now(),
        });

        Self::new(zone, blueprint)
    }
}

// Zone model as stored in DB
#[derive(Debug, Clone)]
pub struct Zone {
    /// Zone id
    pub id: ZoneId,
    /// Unique key
    pub key: String,
    /// Title of the zone
    pub title: String,
    /// Kind of zone
    pub kind: ZoneKind,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl Zone {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let kind_s: &str = row.try_get("kind")?;
        let kind = match kind_s {
            "live" => ZoneKind::Live,
            "draft" => ZoneKind::Draft,
            _ => return Err(DbError::Decode("invalid zone.kind".into())),
        };

        Ok(Zone {
            id: row.try_get("id")?,
            key: row.try_get("key")?,
            title: row.try_get("title")?,
            kind,
            created_at: row.try_get("created_at")?,
        })
    }
}

/// Router that defines how to access zone backends based on zone policy
pub struct ZoneRouter {
    db: Arc<DbBackend>,
    mem: Arc<MemoryBackend>,
}

impl ZoneRouter {
    pub fn new(db: Arc<DbBackend>, mem: Arc<MemoryBackend>) -> Self {
        Self { db, mem }
    }

    pub fn storage_for(&self, zone_ctx: &ZoneContext) -> Arc<dyn StateStorage> {
        match zone_ctx.policy.persistence {
            Persistence::Ephemeral => self.mem.clone(),
            Persistence::Persistent => self.db.clone(),
        }
    }
}

#[async_trait]
pub trait StateStorage: Send + Sync {
    async fn update_zone_room_kv(&self, zone_id: ZoneId, room_id: RoomId, key: &str, value: Value) -> AppResult<bool>;
    async fn update_user_room_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId, key: &str, value: Value) -> AppResult<bool>;

    async fn update_zone_object_kv(&self, zone_id: ZoneId, object_id: ObjectId, key: &str, value: Value) -> AppResult<bool>;
    async fn update_user_object_kv(&self, zone_id: ZoneId, account_id: AccountId, object_id: ObjectId, key: &str, value: Value) -> AppResult<bool>;

    // async fn update_inventory(&mut self, room_id: RoomId, obj_id: ObjectId, qty: i32) -> AppResult<bool>;
    // async fn update_xp(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;
    // async fn update_health(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;
    // async fn update_coins(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;

    async fn set_current_room(&self, zone_id: ZoneId, account_id: AccountId, to_room: RoomId) -> AppResult<()>;
    async fn record_travel(&self, zone_id: ZoneId, account_id: AccountId, from: RoomId, to: RoomId) -> AppResult<()>;
}

// -----------------------------------------------------------------------------------------------
pub struct DbBackend {
    db: Arc<Db>,
}

impl DbBackend {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StateStorage for DbBackend {
    async fn update_zone_room_kv(&self, zone_id: ZoneId, room_id: RoomId, key: &str, value: Value) -> AppResult<bool> {
        let client = self.db.get_client().await?;

        let _ = client.execute(
            "INSERT INTO zone_room_kv (zone_id, room_id, key, value)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (zone_id, room_id, key)
                DO UPDATE SET value = EXCLUDED.value",
            &[&zone_id, &room_id, &key, &value],
        ).await.map_err(DbError::from)?;

        Ok(true)
    }

    async fn update_user_room_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId, key: &str, value: Value) -> AppResult<bool> {
        let client = self.db.get_client().await?;

        let _ = client.execute(
            "INSERT INTO user_room_kv (zone_id, room_id, account_id, key, value)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (zone_id, room_id, account_id, key)
                DO UPDATE SET value = EXCLUDED.value",
            &[&zone_id, &room_id, &account_id, &key, &value],
        ).await.map_err(DbError::from)?;

        Ok(true)
    }

    async fn update_zone_object_kv(&self, zone_id: ZoneId, object_id: ObjectId, key: &str, value: Value) -> AppResult<bool> {
        let client = self.db.get_client().await?;

        let _ = client.execute(
            "INSERT INTO zone_object_kv (zone_id, object_id, key, value)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (zone_id, object_id, key)
                DO UPDATE SET value = EXCLUDED.value",
            &[&zone_id, &object_id, &key, &value],
        ).await.map_err(DbError::from)?;

        Ok(true)
    }

    async fn update_user_object_kv(&self, zone_id: ZoneId, account_id: AccountId, object_id: ObjectId, key: &str, value: Value) -> AppResult<bool> {
        let client = self.db.get_client().await?;

        let _ = client.execute(
            "INSERT INTO user_object_kv (zone_id, account_id, object_id, key, value)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (zone_id, account_id, object_id, key)
                DO UPDATE SET value = EXCLUDED.value",
            &[&zone_id, &account_id, &object_id, &key, &value],
        ).await.unwrap();

        Ok(true)
    }

    async fn set_current_room(&self, zone_id: ZoneId, account_id: AccountId, to_room: RoomId) -> AppResult<()> {
        let client = self.db.get_client().await?;

        let _ = client.execute(
            "UPDATE characters SET room_id = $1
            WHERE zone_id = $2 AND account_id = $3",
            &[&to_room, &zone_id, &account_id],
        ).await.map_err(DbError::from)?;

        Ok(())
    }

    async fn record_travel(&self, _zone_id: ZoneId, _account_id: AccountId, _from: RoomId, _to: RoomId) -> AppResult<()> {
        todo!()
    }
}

// -----------------------------------------------------------------------------------------------
pub struct MemoryBackend {
    zones: DashMap<ZoneId, Arc<MemZone>>,
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self { zones: DashMap::new() }
    }

    fn zone(&self, id: ZoneId) -> Arc<MemZone> {
        self.zones
            .entry(id)
            .or_insert_with(|| Arc::new(MemZone::default()))
            .clone()
    }
}

// enum StringOrVec {
//     Str(String),
//     Vec(Vec<String>),
// }

// impl TryFrom<Value> for StringOrVec {
//     type Error = DomainError;
//
//     fn try_from(v: Value) -> Result<Self, Self::Error> {
//         match v {
//             Value::String(s) => Ok(StringOrVec::Str(s)),
//             Value::Array(arr) => {
//                 let mut out = Vec::with_capacity(arr.len());
//                 for item in arr {
//                     match item {
//                         Value::String(s) => out.push(s),
//                         _ => {},
//                     }
//                 }
//                 Ok(StringOrVec::Vec(out))
//             }
//             _ => Ok(StringOrVec::Str("".into()))
//         }
//     }
// }


#[derive(Default)]
struct MemZone {
    zone_room_kv: DashMap<RoomId, HashMap<String, String>>,
    user_room_kv: DashMap<(RoomId, AccountId), HashMap<String, String>>,

    zone_object_kv: DashMap<ObjectId, HashMap<String, String>>,
    user_object_kv: DashMap<(ObjectId, AccountId), HashMap<String, String>>,

    // room_qty: DashMap<(RoomId, ObjectId), i32>,
    // coins: DashMap<AccountId, i32>,
    // items: DashMap<(AccountId, ObjectId), i32>,
    // health: DashMap<AccountId, i32>,
    // xp: DashMap<AccountId, i32>,

    current_room: DashMap<AccountId, RoomId>,
}

#[async_trait]
impl StateStorage for MemoryBackend {
    async fn update_zone_room_kv(&self, zone_id: ZoneId, room_id: RoomId, key: &str, value: Value) -> AppResult<bool> {
        let zone = self.zone(zone_id);

        let mut inner = zone
            .zone_room_kv
            .entry(room_id)
            .or_insert_with(HashMap::new);
        inner.insert(key.to_string(), serde_to_str(value));

        Ok(true)
    }

    async fn update_user_room_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId, key: &str, value: Value) -> AppResult<bool> {
        let zone = self.zone(zone_id);

        let mut inner = zone
            .user_room_kv
            .entry((room_id, account_id))
            .or_insert_with(HashMap::new);
        inner.insert(key.to_string(), serde_to_str(value));

        Ok(true)
    }

    async fn update_zone_object_kv(&self, zone_id: ZoneId, object_id: ObjectId, key: &str, value: Value) -> AppResult<bool> {
        let zone = self.zone(zone_id);

        let mut inner = zone
            .zone_object_kv
            .entry(object_id)
            .or_insert_with(HashMap::new);
        inner.insert(key.to_string(), serde_to_str(value));

        Ok(true)
    }

    async fn update_user_object_kv(&self, zone_id: ZoneId, account_id: AccountId, object_id: ObjectId, key: &str, value: Value) -> AppResult<bool>{
        let zone = self.zone(zone_id);

        let mut inner = zone
            .user_object_kv
            .entry((object_id, account_id))
            .or_insert_with(HashMap::new);
        inner.insert(key.to_string(), serde_to_str(value));

        Ok(true)
    }

    async fn set_current_room(&self, zone_id: ZoneId, account_id: AccountId, to_room: RoomId) -> AppResult<()> {
        let zone = self.zone(zone_id);
        zone.current_room.insert(account_id, to_room);

        Ok(())
    }

    async fn record_travel(&self, _zone_id: ZoneId, _account_id: AccountId, _from: RoomId, _to: RoomId) -> AppResult<()> {
        // @TODO: We don't track travel history in memory for now
        Ok(())
    }
}