use crate::config::Config;
use crate::db::Db;
use std::collections::BTreeSet;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::models::account::Account;
use crate::db::repo::account::AccountRepo;
use crate::db::repo::db_account::AccountRepository;
use crate::db::repo::db_kv::KvRepository;
use crate::db::repo::db_room::RoomRepository;
use crate::db::repo::kv::KvRepo;
use crate::db::repo::room::RoomRepo;
use crate::services::{AccountService, AuthService, BlueprintService, RoomService};

/// We are entering container / DI territory here. We have to be careful that we don't create
/// circular references.

pub struct Repos {
    pub account: Arc<dyn AccountRepo>,
    pub room: Arc<dyn RoomRepo>,
    pub kv: Arc<dyn KvRepo>,
}

pub struct Services {
    pub auth: Arc<AuthService>,
    pub account: Arc<AccountService>,
    pub blueprint: Arc<BlueprintService>,
    pub room: Arc<RoomService>,
    pub cursor: Arc<CursorService>,
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
            kv: Arc::new(KvRepository::new(db.clone())),
        });

        let services = Arc::new(Services {
            auth: Arc::new(AuthService::new(repos.account.clone())),
            account: Arc::new(AccountService::new(repos.account.clone())),
            blueprint: Arc::new(BlueprintService::new(repos.room.clone())),
            room: Arc::new(RoomService::new(repos.kv.clone())),
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
