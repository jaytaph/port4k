use anyhow::anyhow;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use std::str::FromStr;
use tokio_postgres::NoTls;

use super::Db;

impl Db {
    pub fn new(url: &str) -> anyhow::Result<Self> {
        let cfg = tokio_postgres::Config::from_str(url)?;
        let mgr = Manager::from_config(
            cfg,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );
        let pool = Pool::builder(mgr)
            .max_size(16)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| anyhow!(e))?;
        Ok(Self { pool })
    }
}
