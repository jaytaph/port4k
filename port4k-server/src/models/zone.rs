use crate::db::error::DbError;
use crate::db::{Db, DbResult};
use crate::error::{AppResult, DomainError};
use crate::models::blueprint::Blueprint;
use crate::models::room::ZoneRoomState;
use crate::models::types::{AccountId, ObjectId, RoomId, ZoneId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio_postgres::Row;

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
    Ephemeral,
    Persistent,
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

    pub fn state_for(&self, zone_ctx: &ZoneContext) -> Arc<dyn ZoneState> {
        match zone_ctx.policy.persistence {
            Persistence::Ephemeral => self.mem.clone(),
            Persistence::Persistent => self.db.clone(),
        }
    }
}

#[async_trait]
pub trait ZoneUnitOfWork: Send {
    async fn commit(self: Box<Self>) -> AppResult<()>;
    async fn rollback(self: Box<Self>) -> AppResult<()>;

    async fn update_inventory(&mut self, room_id: RoomId, obj_id: ObjectId, qty: i32) -> AppResult<bool>;
    async fn update_xp(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;
    async fn update_health(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;
    async fn update_coins(&mut self, account_id: AccountId, amount: i32) -> AppResult<bool>;

    async fn set_current_room(&mut self, account_id: AccountId, to_room: RoomId) -> AppResult<()>;
    async fn record_travel(&mut self, account_id: AccountId, from: RoomId, to: RoomId) -> AppResult<()>;
}

#[async_trait]
pub trait ZoneState: Send + Sync {
    async fn begin(&self, zone_ctx: &ZoneContext) -> DbResult<Box<dyn ZoneUnitOfWork>>;
    async fn zone_room_state(
        &self,
        zone_ctx: &ZoneContext,
        room_id: RoomId,
        account_id: AccountId,
    ) -> AppResult<ZoneRoomState>;
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
impl ZoneState for DbBackend {
    async fn begin(&self, zone_ctx: &ZoneContext) -> DbResult<Box<dyn ZoneUnitOfWork>> {
        let client = self.db.get_client().await?;

        Ok(Box::new(DbUow {
            zone_id: zone_ctx.zone.id,
            client,
            pending: Pending::default(),
        }))
    }

    async fn zone_room_state(
        &self,
        zone_ctx: &ZoneContext,
        room_id: RoomId,
        account_id: AccountId,
    ) -> AppResult<ZoneRoomState> {
        let client = self.db.get_client().await?;

        // No per-room quantities yet
        let room_qty = Vec::new();

        // per-account, per-zone state
        let row = client
            .query_opt(
                "SELECT coins, health, xp, current_room_id
             FROM account_zone_state
             WHERE account_id = $1 AND zone_id = $2",
                &[&account_id, &zone_ctx.zone.id],
            )
            .await
            .map_err(DbError::from)?;

        let (coins, health, xp, current_room) = if let Some(r) = row {
            (
                r.get::<_, i32>(0),
                r.get::<_, i32>(1),
                r.get::<_, i32>(2),
                r.get::<_, Option<RoomId>>(3),
            )
        } else {
            (0, 100, 0, None)
        };

        // inventory (bag)
        let inv_rows = client
            .query(
                "SELECT object_id, qty
             FROM account_zone_items
             WHERE account_id=$1 AND zone_id=$2
             ORDER BY object_id",
                &[&account_id, &zone_ctx.zone.id],
            )
            .await
            .map_err(DbError::from)?;

        let inventory = inv_rows
            .into_iter()
            .map(|r| (r.get::<_, ObjectId>(0), r.get::<_, i32>(1)))
            .collect::<Vec<_>>();

        let raw = RawState {
            room_qty,
            coins,
            health,
            xp,
            items: inventory,
            current_room,
        };

        Ok(compose_zone_room_state(zone_ctx, room_id, account_id, raw))
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
        let tx = self.client.build_transaction().start().await.map_err(DbError::from)?;

        // 1) validate and apply room decrements (with row lock)
        for (room_id, obj_id, need) in &self.pending.decs {
            // SELECT qty FOR UPDATE to lock row
            let row = tx
                .query_opt(
                    "SELECT qty FROM zone_room_qty
                 WHERE zone_id = $1 AND room_id = $2 AND obj_id = $3
                 FOR UPDATE",
                    &[&self.zone_id, room_id, obj_id],
                )
                .await
                .map_err(DbError::from)?;
            let have: i32 = row.as_ref().map(|r| r.get(0)).unwrap_or(0);
            if have < *need {
                return Err(DomainError::InsufficientQuantity {
                    room_id: *room_id,
                    obj_id: *obj_id,
                    have,
                    need: *need,
                });
            }
            // UPDATE existing row (we know it exists when have>0; if have==0 & need==0, skip)
            if have > 0 && *need > 0 {
                tx.execute(
                    "UPDATE zone_room_qty
                    SET qty = qty - $4
                    WHERE zone_id = $1 AND room_id = $2 AND obj_id = $3",
                    &[&self.zone_id, room_id, obj_id, need],
                )
                .await
                .map_err(DbError::from)?;
            }
        }

        // 2) apply coin deltas (PER ACCOUNT, PER ZONE)
        for (acct, amt) in &self.pending.coin_adds {
            // UPSERT add
            tx.execute(
                "INSERT INTO account_zone_state (account_id, zone_id, coins)
                VALUES ($1, $2, $3)
                ON CONFLICT (account_id, zone_id)
                DO UPDATE SET coins = account_zone_state.coins + EXCLUDED.coins",
                &[acct, &self.zone_id, amt],
            )
            .await
            .map_err(DbError::from)?;
        }

        // 3) apply health deltas
        for (acct, d) in &self.pending.health_deltas {
            tx.execute(
                "INSERT INTO account_zone_state (account_id, zone_id, health)
                VALUES ($1, $2, $3)
                ON CONFLICT (account_id, zone_id)
                DO UPDATE SET health = account_zone_state.health + EXCLUDED.health",
                &[acct, &self.zone_id, d],
            )
            .await
            .map_err(DbError::from)?;
        }

        // 4) apply xp deltas
        for (acct, amt) in &self.pending.xp_adds {
            tx.execute(
                "INSERT INTO account_zone_state (account_id, zone_id, xp)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (account_id, zone_id)
                 DO UPDATE SET xp = account_zone_state.xp + EXCLUDED.xp",
                &[acct, &self.zone_id, amt],
            )
            .await
            .map_err(DbError::from)?;
        }

        // 5) apply item adds (per-account, PER-ZONE inventory)
        for (acct, obj, qty) in &self.pending.item_adds {
            tx.execute(
                "INSERT INTO account_zone_items (account_id, zone_id, object_id, qty)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (account_id, zone_id, object_id)
                 DO UPDATE SET qty = account_zone_items.qty + EXCLUDED.qty",
                &[acct, &self.zone_id, obj, qty],
            )
            .await
            .map_err(DbError::from)?;
            // clamp to >= 0 (delete if <= 0)
            tx.execute(
                "DELETE FROM account_zone_items
                WHERE account_id=$1 AND zone_id=$2 AND object_id=$3 AND qty <= 0",
                &[acct, &self.zone_id, obj],
            )
            .await
            .map_err(DbError::from)?;
        }

        // 6) apply movement (set_current_room) PER ZONE
        for (acct, to_room) in &self.pending.moves {
            tx.execute(
                "INSERT INTO account_zone_state (account_id, zone_id, current_room_id)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (account_id, zone_id)
                 DO UPDATE SET current_room_id = EXCLUDED.current_room_id",
                &[acct, &self.zone_id, to_room],
            )
            .await
            .map_err(DbError::from)?;
        }

        // // 7) audit travels (optional)
        // for (acct, from, to) in &self.pending.travels {
        //     tx.execute(
        //         "INSERT INTO travel_audit (account_id, zone_id, from_room_id, to_room_id, at)
        //          VALUES ($1, $2, $3, $4, now())",
        //         &[acct, &self.zone_id, from, to],
        //     ).await.map_err(DbError::from)?;
        // }

        tx.commit().await.map_err(DbError::from)?;
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

    async fn set_current_room(&mut self, account_id: AccountId, to_room: RoomId) -> AppResult<()> {
        self.pending.moves.push((account_id, to_room));
        Ok(())
    }

    // [DbUow::record_travel]
    async fn record_travel(&mut self, account_id: AccountId, from: RoomId, to: RoomId) -> AppResult<()> {
        self.pending.travels.push((account_id, from, to));
        Ok(())
    }
}

// -----------------------------------------------------------------------------------------------
pub struct MemoryBackend {
    zones: DashMap<ZoneId, Arc<MemZone>>,
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

#[derive(Default)]
struct MemZone {
    room_qty: DashMap<(RoomId, ObjectId), i32>,
    coins: DashMap<AccountId, i32>,
    items: DashMap<(AccountId, ObjectId), i32>,
    health: DashMap<AccountId, i32>,
    xp: DashMap<AccountId, i32>,
    current_room: DashMap<AccountId, RoomId>,

    commit_lock: Mutex<()>,
}

#[derive(Clone, Debug, Default)]
struct RawState {
    room_qty: Vec<(ObjectId, i32)>,
    coins: i32,
    health: i32,
    xp: i32,
    items: Vec<(ObjectId, i32)>,
    current_room: Option<RoomId>,
}

#[async_trait]
impl ZoneState for MemoryBackend {
    async fn begin(&self, zone_ctx: &ZoneContext) -> DbResult<Box<dyn ZoneUnitOfWork>> {
        Ok(Box::new(MemUow {
            z: self.zone(zone_ctx.zone.id),
            pending: Default::default(),
        }))
    }

    async fn zone_room_state(
        &self,
        zone_ctx: &ZoneContext,
        room_id: RoomId,
        account_id: AccountId,
    ) -> AppResult<ZoneRoomState> {
        let z = self.zone(zone_ctx.zone.id);

        let mut room_qty = Vec::new();
        for entry in z.room_qty.iter() {
            let ((r, obj), qty) = (*entry.key(), *entry.value());
            if r == room_id {
                room_qty.push((obj, qty));
            }
        }

        // Per account
        let coins = z.coins.get(&account_id).map(|v| *v).unwrap_or(0);
        let health = z.health.get(&account_id).map(|v| *v).unwrap_or(0);
        let xp = z.xp.get(&account_id).map(|v| *v).unwrap_or(0);
        let current_room = z.current_room.get(&account_id).map(|v| *v);
        let items = z
            .items
            .iter()
            .filter_map(|e| {
                let ((acct, obj), qty) = (*e.key(), *e.value());
                if acct == account_id { Some((obj, qty)) } else { None }
            })
            .collect::<Vec<_>>();

        let raw = RawState {
            room_qty,
            coins,
            health,
            xp,
            items,
            current_room,
        };

        Ok(compose_zone_room_state(zone_ctx, room_id, account_id, raw))
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

    moves: Vec<(AccountId, RoomId)>,
    travels: Vec<(AccountId, RoomId, RoomId)>,
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
                return Err(DomainError::InsufficientQuantity {
                    room_id: *room,
                    obj_id: *obj,
                    have: cur,
                    need: *qty,
                });
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
            *self.z.health.entry(*acct).or_insert(0) += *d;
        }
        for (acct, amt) in &self.pending.xp_adds {
            *self.z.xp.entry(*acct).or_insert(0) += *amt;
        }

        // apply movement
        for (acct, to_room) in &self.pending.moves {
            self.z.current_room.insert(*acct, *to_room);
        }
        // for (acct, from, to) in &self.pending.travels {
        //     let mut v = self.z.trail.entry(*acct).or_insert_with(Vec::new);
        //     v.push((*from, *to));
        // }

        Ok(())
    }

    async fn rollback(self: Box<Self>) -> AppResult<()> {
        Ok(())
    }

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

    async fn set_current_room(&mut self, acct: AccountId, to_room: RoomId) -> AppResult<()> {
        self.pending.moves.push((acct, to_room));
        Ok(())
    }

    async fn record_travel(&mut self, acct: AccountId, from: RoomId, to: RoomId) -> AppResult<()> {
        self.pending.travels.push((acct, from, to));
        Ok(())
    }
}

fn compose_zone_room_state(
    _zone_ctx: &ZoneContext,
    room_id: RoomId,
    _account_id: AccountId,
    raw: RawState,
) -> ZoneRoomState {
    let room_qty: HashMap<ObjectId, i32> = raw.room_qty.into_iter().collect();
    let discovered_objs: HashSet<ObjectId> = raw
        .items
        .into_iter()
        .filter(|(_obj, qty)| *qty > 0)
        .map(|(obj, _qty)| obj)
        .collect();

    ZoneRoomState {
        // zone_id: zone_ctx.zone.id,
        // account_id,
        room_id,
        // coins: raw.coins,
        // health: raw.health,
        // xp: raw.xp,
        // current_room: raw.current_room,
        // room_qty,
        // trail: vec![], // TODO
        all_objects: room_qty,
        discovered_objects: discovered_objs,
    }
}
