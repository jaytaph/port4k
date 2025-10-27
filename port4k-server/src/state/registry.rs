use crate::config::Config;
use crate::db::Db;
use crate::db::repo::{AccountRepo, AccountRepository, RoomRepository, UserRepo, UserRepository, ZoneRepository};
use crate::db::repo::RoomRepo;
use crate::db::repo::ZoneRepo;
use crate::models::account::Account;
use crate::models::zone::{DbBackend, MemoryBackend, ZoneRouter};
use crate::services::{
    AccountService, AuthService, BlueprintService, CursorService, NavigatorService, RoomService, ZoneService,
};
use parking_lot::RwLock;
use std::collections::BTreeSet;
use std::sync::Arc;

pub struct Repos {
    pub account: Arc<dyn AccountRepo>,
    pub room: Arc<dyn RoomRepo>,
    pub user: Arc<dyn UserRepo>,
    pub zone: Arc<dyn ZoneRepo>,
}

pub struct Services {
    pub auth: Arc<AuthService>,
    pub account: Arc<AccountService>,
    pub blueprint: Arc<BlueprintService>,
    pub room: Arc<RoomService>,
    pub cursor: Arc<CursorService>,
    pub navigator: Arc<NavigatorService>,
    pub zone: Arc<ZoneService>,
}

pub struct Registry {
    pub db: Arc<Db>,
    pub repos: Arc<Repos>,
    pub services: Arc<Services>,
    pub config: Arc<Config>,
    pub online: RwLock<BTreeSet<String>>,
    pub zone_router: Arc<ZoneRouter>,
}

impl Registry {
    pub fn new(db: Arc<Db>, config: Arc<Config>) -> Self {
        let repos = Arc::new(Repos {
            account: Arc::new(AccountRepository::new(db.clone())),
            room: Arc::new(RoomRepository::new(db.clone())),
            user: Arc::new(UserRepository::new(db.clone())),
            zone: Arc::new(ZoneRepository::new(db.clone())),
        });

        let zone_db = Arc::new(DbBackend::new(db.clone()));
        let zone_mem = Arc::new(MemoryBackend::new());
        let zone_router = Arc::new(ZoneRouter::new(zone_db, zone_mem));

        let blueprint_service = Arc::new(BlueprintService::new(repos.room.clone()));
        let room_service = Arc::new(RoomService::new(repos.room.clone(), repos.zone.clone(), repos.user.clone()));

        let services = Arc::new(Services {
            auth: Arc::new(AuthService::new(repos.account.clone())),
            account: Arc::new(AccountService::new(repos.account.clone())),
            blueprint: blueprint_service.clone(),
            room: room_service.clone(),
            cursor: Arc::new(CursorService::new(zone_router.clone(), room_service.clone())),
            navigator: Arc::new(NavigatorService::new(zone_router.clone())),
            zone: Arc::new(ZoneService::new(repos.zone.clone(), room_service.clone())),
        });

        Self {
            db,
            config,
            repos,
            services,
            online: RwLock::new(BTreeSet::new()),
            zone_router: zone_router.clone(),
        }
    }

    pub async fn set_online(&self, account: &Account, online: bool) {
        let mut g = self.online.write();
        if online {
            g.insert(account.username.clone());
        } else {
            g.remove(&account.username);
        }
    }

    pub async fn who(&self) -> Vec<String> {
        self.online.read().iter().cloned().collect()
    }
}
