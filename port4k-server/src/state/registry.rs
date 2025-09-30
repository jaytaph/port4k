use crate::db::Db;
use port4k_core::Username;
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::config::Config;

#[derive(Debug)]
pub struct Registry {
    pub db: Db,
    pub config: Arc<Config>,
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

    pub async fn set_online(&self, name: &Username, online: bool) {
        let mut g = self.online.write().await;
        if online {
            g.insert(name.0.clone());
        } else {
            g.remove(&name.0);
        }
    }

    pub async fn who(&self) -> Vec<String> {
        self.online.read().await.iter().cloned().collect()
    }

    pub async fn user_exists(&self, name: &Username) -> bool {
        self.db.user_exists(&name.0).await.unwrap_or(false)
    }
}
