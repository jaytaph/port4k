use crate::config::Config;
use crate::db::Db;
use crate::db::repo::{RealmRepo, RealmRepository};
use crate::db::repo::{AccountRepo, AccountRepository, RoomRepository, UserRepo, UserRepository};
use crate::db::repo::{InventoryRepo, InventoryRepository, RoomRepo};
use crate::models::account::Account;
use crate::services::{
    AccountService, BlueprintService, InventoryService, RoomService, RealmService
};
use parking_lot::RwLock;
use std::collections::BTreeSet;
use std::sync::Arc;

pub struct Repos {
    pub account: Arc<dyn AccountRepo>,
    pub room: Arc<dyn RoomRepo>,
    pub user: Arc<dyn UserRepo>,
    pub inventory: Arc<dyn InventoryRepo>,
    pub realm: Arc<dyn RealmRepo>,
}

pub struct Services {
    pub account: Arc<AccountService>,
    pub blueprint: Arc<BlueprintService>,
    pub room: Arc<RoomService>,
    pub realm: Arc<RealmService>,
    pub inventory: Arc<InventoryService>,
}

pub struct Registry {
    pub db: Arc<Db>,
    pub repos: Arc<Repos>,
    pub services: Arc<Services>,
    pub config: Arc<Config>,
    pub online: RwLock<BTreeSet<String>>,
}

impl Registry {
    pub fn new(db: Arc<Db>, config: Arc<Config>) -> Self {
        let repos = Arc::new(Repos {
            account: Arc::new(AccountRepository::new(db.clone())),
            room: Arc::new(RoomRepository::new(db.clone())),
            user: Arc::new(UserRepository::new(db.clone())),
            inventory: Arc::new(InventoryRepository::new(db.clone())),
            realm: Arc::new(RealmRepository::new(db.clone())),
        });

        let inventory_service = Arc::new(InventoryService::new(repos.inventory.clone()));
        let blueprint_service = Arc::new(BlueprintService::new(repos.room.clone()));
        let room_service = Arc::new(RoomService::new(
            repos.room.clone(),
            repos.realm.clone(),
            repos.user.clone(),
            repos.account.clone(),
            inventory_service.clone(),
        ));

        let services = Arc::new(Services {
            account: Arc::new(AccountService::new(repos.account.clone())),
            blueprint: blueprint_service.clone(),
            inventory: inventory_service,
            room: room_service.clone(),
            realm: Arc::new(RealmService::new(repos.realm.clone(), repos.user.clone())),
        });

        Self {
            db,
            config,
            repos,
            services,
            online: RwLock::new(BTreeSet::new()),
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
