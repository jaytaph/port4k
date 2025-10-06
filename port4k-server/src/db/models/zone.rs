use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use async_trait::async_trait;
use tokio_postgres::Row;
use toml::value::Datetime;
use crate::db::models::blueprint::Blueprint;
use crate::db::models::room::RoomView;
use crate::db::types::{AccountId, ObjectId, RoomId, ZoneId};

#[derive(Clone, Debug)]
pub enum ZoneKind {
    Live,
    Draft,
    Test { owner: AccountId },
}

#[derive(Clone, Debug)]
enum Persistence { Ephemeral, Persistent }

impl Persistence {
    #[inline] pub fn is_ephemeral(&self) -> bool { matches!(self, Persistence::Ephemeral) }
    #[inline] pub fn is_persistent(&self) -> bool { matches!(self, Persistence::Persistent) }
}

#[derive(Clone, Debug)]
pub struct ZonePolicy {
    pub persistence: Persistence,
}

impl ZonePolicy {
    pub fn for_kind(kind: &ZoneKind) -> Self {
        match kind {
            ZoneKind::Live => ZonePolicy { persistence: Persistence::Persistent },
            ZoneKind::Draft => ZonePolicy { persistence: Persistence::Ephemeral },
            ZoneKind::Test { .. } => ZonePolicy { persistence: Persistence::Ephemeral },
        }
    }
}

#[derive(Clone, Debug)]
pub struct ZoneRef {
    pub id: ZoneId,
    pub kind: ZoneKind,
    pub policy: ZonePolicy,
    pub blueprint: Blueprint,
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub id: ZoneId,
    pub key: String,
    pub title: String,
    pub kind: ZoneKind,
    pub created_at: Datetime,
}

impl Zone {
    pub fn from_row(row: Row) -> Self {
        let kind_str: String = row.get("kind");
        let kind = match kind_str.as_str() {
            "live" => ZoneKind::Live,
            "draft" => ZoneKind::Draft,
            _ => ZoneKind::Draft, // Default to Draft if unknown
        };

        Zone {
            id: row.get::<_, ZoneId>("id"),
            key: row.get("key"),
            title: row.get("title"),
            kind,
            created_at: row.get::<_, Datetime>("created_at"),
        }
    }
}

pub struct ZoneRouter {
    db: Arc<DbBackend>,
    mem: Arc<MemoryBackend>
}

impl ZoneRouter {
    pub fn new(db: Arc<DbBackend>, mem: Arc<MemoryBackend>) -> Self {
        Self { db, mem }
    }

    pub fn backend_for(&self, zone_ref: &ZoneRef) -> Arc<dyn ZoneBackend> {
        match zone_ref.policy.persistence {
            Persistence::Ephemeral => self.mem.clone(),
            Persistence::Persistent => self.db.clone(),
        }
    }

    pub fn view_repo_for(&self, zone_ref: &ZoneRef) -> Arc<dyn ZoneViewRepo> {
        match zone_ref.policy.persistence {
            Persistence::Ephemeral => self.mem.clone(),
            Persistence::Persistent => self.db.clone(),
        }
    }
}

#[async_trait]
pub trait ZoneViewRepo: Send + Sync {
    async fn room_view(&self, zone: &ZoneRef, room_id: RoomId, width: u16) -> anyhow::Result<RoomView>;
}

#[async_trait]
pub trait ZoneUnitOfWork: Send {
    async fn commit(self: Box<Self>) -> anyhow::Result<()>;
    async fn rollback(self: Box<Self>) -> anyhow::Result<()>;

    async fn update_inventory(&mut self, room_id: RoomId, obj_id: ObjectId, qty: i32) -> anyhow::Result<bool>;
    async fn update_xp(&mut self, account_id: AccountId, amount: i32) -> anyhow::Result<bool>;
    async fn update_health(&mut self, account_id: AccountId, amount: i32) -> anyhow::Result<bool>;
    async fn update_coins(&mut self, account_id: AccountId, amount: i32) -> anyhow::Result<bool>;
}

#[async_trait]
pub trait ZoneBackend: Send + Sync {
    async fn begin(&self, zone_ref: &ZoneRef) -> anyhow::Result<Box<dyn ZoneUnitOfWork>>;
}

// -----------------------------------------------------------------------------------------------
pub struct DbBackend {}

impl DbBackend {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ZoneBackend for DbBackend {
    async fn begin(&self, zone_id: ZoneId) -> anyhow::Result<Box<dyn ZoneUnitOfWork>> {
        Ok(Box::new(DbUow {}))
    }
}

#[async_trait]
impl ZoneViewRepo for DbBackend {
    async fn room_view(&self, zone: &ZoneRef, room_id: RoomId, width: u16) -> anyhow::Result<RoomView> {
        let _ = zone;
        let _ = room_id;
        let _ = width;
        todo!()
    }
}

// -----------------------------------------------------------------------------------------------
struct DbUow {}

#[async_trait]
impl ZoneUnitOfWork for DbUow {
    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        todo!()
    }
    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        todo!()
    }

    async fn update_inventory(&mut self, room_id: RoomId, obj_id: ObjectId, qty: i32) -> anyhow::Result<bool> {
        todo!()
    }
    async fn update_xp(&mut self, account_id: AccountId, amount: i32) -> anyhow::Result<bool> {
        todo!()
    }
    async fn update_health(&mut self, account_id: AccountId, amount: i32) -> anyhow::Result<bool> {
        todo!()
    }
    async fn update_coins(&mut self, account_id: AccountId, amount: i32) -> anyhow::Result<bool> {
        todo!()
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
        self.zones.entry(id).or_insert_with(|| Arc::new(MemZone::default)).clone()
    }
}

#[derive(Default)]
struct MemZone {
    room_qty: DashMap<(RoomId, ObjectId), i32>,
    coins: DashMap<AccountId, i32>,
    items: DashMap<(AccountId, ObjectId), i32>,
    commit_lock: Mutex<()>,
}

#[async_trait]
impl ZoneBackend for MemoryBackend {
    async fn begin(&self, zone_ref: &ZoneRef) -> anyhow::Result<Box<dyn ZoneUnitOfWork>> {
        Ok(Box::new(MemUow { z: self.zone(zone_ref.id), pending: Default::default() }))
    }
}

#[async_trait]
impl ZoneViewRepo for MemoryBackend {
    async fn room_view(&self, zone_ref: &ZoneRef, room_id: RoomId, width: u16) -> anyhow::Result<RoomView> {
        let _ = zone_ref;
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
    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        let _g = self.z.commit_lock.lock();

        // validate decs
        for (room, obj, qty) in &self.pending.decs {
            let cur = *self.z.room_qty.get(&(*room, *obj)).unwrap_or(&0);
            if cur < *qty { anyhow::bail!("not enough qty"); }
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
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> { Ok(()) }

    async fn update_inventory(&mut self, room_id: RoomId, obj: ObjectId, qty: i32) -> anyhow::Result<bool> {
        self.pending.decs.push((room_id, obj, qty));
        Ok(true) // final check in commit
    }
    async fn update_coins(&mut self, acct: AccountId, amt: i32) -> anyhow::Result<()> {
        self.pending.coin_adds.push((acct, amt)); Ok(())
    }
    async fn update_health(&mut self, acct: AccountId, d: i32) -> anyhow::Result<()> {
        self.pending.health_deltas.push((acct, d)); Ok(())
    }
    async fn update_xp(&mut self, acct: AccountId, amt: i32) -> anyhow::Result<()> {
        self.pending.xp_adds.push((acct, amt)); Ok(())
    }
}