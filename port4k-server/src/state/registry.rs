use crate::config::Config;
use crate::db::Db;
use port4k_core::Username;
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::db::models::account::Account;

#[derive(Debug)]
pub struct Registry {
    /// Database
    pub db: Db,
    /// Global Configuration
    pub config: Arc<Config>,
    /// List of online players
    pub online: RwLock<BTreeSet<String>>,
}

impl Registry {
    pub fn new(db: Db, config: Arc<Config>) -> Self {
        Self {
            db,
            config,
            online: RwLock::new(BTreeSet::new()),
        }
    }

    pub async fn set_online(&self, account: &Account, online: bool) {
        let mut g = self.online.write().await;
        if online {
            g.insert(account.username.clone());
        } else {
            g.remove(&account.username);
        }
    }

    pub async fn who(&self) -> Vec<String> {
        self.online.read().await.iter().cloned().collect()
    }

    pub async fn user_exists(&self, name: &Username) -> bool {
        self.db.user_exists(&name.0).await.unwrap_or(false)
    }
}
