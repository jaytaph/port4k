use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio_postgres::Row;
use crate::db::{Db, DbError, DbResult};
use crate::error::{AppError, AppResult};
use crate::models::blueprint::Blueprint;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, ObjectId, RoomId, ZoneId};

/// Type of zone defines what is allowed and how it is persisted.
#[derive(Clone, Debug)]
pub enum ZoneKind {
    Live,
    Draft,
    Test { owner: AccountId },
}

/// How the zone is persisted.
#[derive(Clone, Debug)]
enum Persistence { Ephemeral, Persistent }

impl Persistence {
    #[inline] pub fn is_ephemeral(&self) -> bool { matches!(self, Persistence::Ephemeral) }
    #[inline] pub fn is_persistent(&self) -> bool { matches!(self, Persistence::Persistent) }
}

/// Total zone policy (for now, just persistence)
#[derive(Clone, Debug)]
pub struct ZonePolicy {
    pub persistence: Persistence,
}

impl ZonePolicy {
    pub fn for_kind(kind: &ZoneKind) -> Self {
        match kind {
            ZoneKind::Live => ZonePolicy { persistence: Persistence::Persistent },
            ZoneKind::Draft => ZonePolicy { persistence: Persistence::Persistent },
            ZoneKind::Test { .. } => ZonePolicy { persistence: Persistence::Ephemeral },
        }
    }
}

/// Zone context
#[derive(Clone, Debug)]
pub struct ZoneContext {
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
        Self { zone, kind, policy, blueprint }
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
            "live"  => ZoneKind::Live,
            "draft" => ZoneKind::Draft,
            _ => return Err(DbError::Decode("invalid zone.kind")),
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
    mem: Arc<MemoryBackend>
}

impl ZoneRouter {
    pub fn new(db: Arc<DbBackend>, mem: Arc<MemoryBackend>) -> Self {
        Self { db, mem }
    }

    pub fn backend_for(&self, zone_ctx: &ZoneContext) -> Arc<dyn ZoneBackend> {
        match zone_ctx.policy.persistence {
            Persistence::Ephemeral => self.mem.clone(),
            Persistence::Persistent => self.db.clone(),
        }
    }

    pub fn view_repo_for(&self, zone_ctx: &ZoneContext) -> Arc<dyn ZoneViewRepo> {
        match zone_ctx.policy.persistence {
            Persistence::Ephemeral => self.mem.clone(),
            Persistence::Persistent => self.db.clone(),
        }
    }
}

#[async_trait]
pub trait ZoneViewRepo: Send + Sync {
    async fn room_view(&self, zone_ctx: &ZoneContext, room_id: RoomId, width: u16) -> DbResult<RoomView>;
}

#[async_trait]
pub trait ZoneUnitOfWork: Send {
    async fn commit(self: Box<Self>) -> AppResult<()>;
    async fn rollback(self: Box<Self>) -> AppResult<()>;

    async fn update_inventory(&mut self, room_id: RoomId, obj_id: ObjectId, qty: i32) -> AppResult<bool>;
    async fn update_xp(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;
    async fn update_health(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;
    async fn update_coins(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;
}

#[async_trait]
pub trait ZoneBackend: Send + Sync {
    async fn begin(&self, zone_ctx: &ZoneContext) -> DbResult<Box<dyn ZoneUnitOfWork>>;
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
impl ZoneBackend for DbBackend {
    async fn begin(&self, zone_ctx: &ZoneContext) -> DbResult<Box<dyn ZoneUnitOfWork>> {
        let client = self.db.get_client().await?;

        Ok(Box::new(DbUow {
            zone_id: zone_ctx.zone.id,
            client,
            pending: Pending::default(),
        }))
    }
}

#[async_trait]
impl ZoneViewRepo for DbBackend {
    async fn room_view(&self, zone: &ZoneContext, room_id: RoomId, width: u16) -> DbResult<RoomView> {
        let _ = zone;
        let _ = room_id;
        let _ = width;
        todo!()
    }
}

// -----------------------------------------------------------------------------------------------
struct DbUow {
    zone_id: ZoneId,
    client: deadpool_postgres::Client,
    pending: Pending,
}

#[async_trait]
impl ZoneUnitOfWork for DbUow {
    async fn commit(mut self: Box<Self>) -> AppResult<()> {
        let tx = self.client.build_transaction().start().await?;

        // 1) validate and apply room decrements (with row lock)
        for (room_id, obj_id, need) in &self.pending.decs {
            // SELECT qty FOR UPDATE to lock row
            let row = tx.query_opt(
                "SELECT qty FROM zone_room_qty
                 WHERE zone_id = $1 AND room_id = $2 AND obj_id = $3
                 FOR UPDATE",
                &[&self.zone_id, room_id, obj_id],
            ).await?;
            let have: i32 = row.as_ref().map(|r| r.get(0)).unwrap_or(0);
            if have < *need {
                return Err(AppError::InsufficientQuantity {
                    room_id: *room_id, obj_id: *obj_id, have, need: *need
                });
            }
            // UPDATE existing row (we know it exists when have>0; if have==0 & need==0, skip)
            if have > 0 && *need > 0 {
                tx.execute(
                    "UPDATE zone_room_qty
                     SET qty = qty - $4
                     WHERE zone_id = $1 AND room_id = $2 AND obj_id = $3",
                    &[&self.zone_id, room_id, obj_id, need],
                ).await?;
            }
        }

        // 2) apply coin deltas
        for (acct, amt) in &self.pending.coin_adds {
            tx.execute(
                "UPDATE accounts SET coins = coins + $2 WHERE id = $1",
                &[acct, amt],
            ).await?;
        }

        // 3) apply health deltas
        for (acct, d) in &self.pending.health_deltas {
            tx.execute(
                "UPDATE accounts SET health = health + $2 WHERE id = $1",
                &[acct, d],
            ).await?;
        }

        // 4) apply xp deltas
        for (acct, amt) in &self.pending.xp_adds {
            tx.execute(
                "UPDATE accounts SET xp = xp + $2 WHERE id = $1",
                &[acct, amt],
            ).await?;
        }

        // 5) apply item adds (per-account inventory)
        // NOTE: allows negative qty if you ever push negative deltas.
        for (acct, obj, qty) in &self.pending.item_adds {
            tx.execute(
                "INSERT INTO account_items (account_id, object_id, qty)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (account_id, object_id)
                 DO UPDATE SET qty = account_items.qty + EXCLUDED.qty",
                &[acct, obj, qty],
            ).await?;
            // Optional: clamp to >= 0 (delete if <= 0)
            tx.execute(
                "DELETE FROM account_items WHERE account_id=$1 AND object_id=$2 AND qty <= 0",
                &[acct, obj],
            ).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn rollback(mut self: Box<Self>) -> AppResult<()> {
        // No need for rollback, as the transaction is not really started until commit
        Ok(())
    }

    async fn update_inventory(&mut self, room_id: RoomId, obj_id: ObjectId, qty: i32) -> AppResult<bool> {
        self.pending.decs.push((room_id, obj_id, qty));
        Ok(true)
    }

    async fn update_xp(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool> {
        self.pending.xp_adds.push((account_id, amount));
        Ok(true)
    }

    async fn update_health(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool> {
        self.pending.health_deltas.push((account_id, amount));
        Ok(true)
    }

    async fn update_coins(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool> {
        self.pending.coin_adds.push((account_id, amount));
        Ok(true)
    }
}

// -----------------------------------------------------------------------------------------------
pub struct MemoryBackend {
    zones: DashMap<ZoneId, Arc<MemZone>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self {
            zones: DashMap::new(),
        }
    }

    fn zone(&self, id: ZoneId) -> Arc<MemZone> {
        self.zones.entry(id).or_insert_with(|| Arc::new(MemZone::default())).clone()
    }
}

#[derive(Default)]
struct MemZone {
    room_qty: DashMap<(RoomId, ObjectId), i32>,
    coins: DashMap<AccountId, i32>,
    items: DashMap<(AccountId, ObjectId), i32>,
    health: DashMap<AccountId, i32>,
    xp: DashMap<AccountId, i32>,
    commit_lock: Mutex<()>,
}

#[async_trait]
impl ZoneBackend for MemoryBackend {
    async fn begin(&self, zone_ctx: &ZoneContext) -> DbResult<Box<dyn ZoneUnitOfWork>> {
        Ok(Box::new(MemUow { z: self.zone(zone_ctx.zone.id), pending: Default::default() }))
    }
}

#[async_trait]
impl ZoneViewRepo for MemoryBackend {
    async fn room_view(&self, zone_ctx: &ZoneContext, room_id: RoomId, width: u16) -> DbResult<RoomView> {
        let _ = zone_ctx;
        let _ = room_id;
        let _width = width;

        todo!()
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Default)]
struct Pending {
    decs: Vec<(RoomId, ObjectId, i32)>,
    coin_adds: Vec<(AccountId, i32)>,
    item_adds: Vec<(AccountId, ObjectId, i32)>,
    health_deltas: Vec<(AccountId, i32)>,
    xp_adds: Vec<(AccountId, i32)>,
}

struct MemUow {
    z: Arc<MemZone>,
    pending: Pending,
}

#[async_trait]
impl ZoneUnitOfWork for MemUow {
    async fn commit(self: Box<Self>) -> AppResult<()> {
        let _g = self.z.commit_lock.lock();

        // validate decs
        for (room, obj, qty) in &self.pending.decs {
            let cur = self.z.room_qty.get(&(*room, *obj)).map(|v| *v).unwrap_or(0);
            if cur < *qty {
                return Err(AppError::InsufficientQuantity { room_id: *room, obj_id: *obj, have: cur, need: *qty });
            }
        }
        // apply decs
        for (room, obj, qty) in &self.pending.decs {
            let mut entry = self.z.room_qty.entry((*room, *obj)).or_insert(0);
            *entry -= *qty;
        }
        // apply coins/items
        for (acct, amt) in &self.pending.coin_adds {
            *self.z.coins.entry(*acct).or_insert(0) += *amt;
        }
        for (acct, obj, qty) in &self.pending.item_adds {
            *self.z.items.entry((*acct, *obj)).or_insert(0) += *qty;
        }
        for (acct, d) in &self.pending.health_deltas {
            *self.z.items.entry((*acct, ObjectId::new())).or_insert(0) += *d; // placeholder
        }
        for (acct, amt) in &self.pending.xp_adds {
            *self.z.xp.entry(*acct).or_insert(0) += *amt;
        }
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> AppResult<()> { Ok(()) }

    async fn update_inventory(&mut self, room_id: RoomId, obj: ObjectId, qty: i32) -> AppResult<bool> {
        self.pending.decs.push((room_id, obj, qty));
        Ok(true) // final check in commit
    }

    async fn update_xp(&mut self, acct: AccountId, amt: i32) -> AppResult<bool> {
        self.pending.xp_adds.push((acct, amt));
        Ok(true)
    }

    async fn update_health(&mut self, acct: AccountId, d: i32) -> AppResult<bool> {
        self.pending.health_deltas.push((acct, d));
        Ok(true)
    }

    async fn update_coins(&mut self, acct: AccountId, amt: i32) -> AppResult<bool> {
        self.pending.coin_adds.push((acct, amt));
        Ok(true)
    }
}