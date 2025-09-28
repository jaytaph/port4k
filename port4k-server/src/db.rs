use std::str::FromStr;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

#[derive(Clone, Debug)]
pub struct Db {
    pool: Pool,
}

impl Db {
    pub fn new(url: &str) -> anyhow::Result<Self> {
        let cfg = tokio_postgres::Config::from_str(url)?;
        let mgr = Manager::from_config(cfg, NoTls, ManagerConfig { recycling_method: RecyclingMethod::Fast });
        let pool = Pool::builder(mgr).max_size(16).runtime(Runtime::Tokio1).build().expect("build pool");
        Ok(Self { pool })
    }

    /// Run embedded SQL migrations (idempotent).
    pub async fn init(&self) -> anyhow::Result<()> {
        let mut client = self.pool.get().await?;
        embedded::migrations::runner()
            .run_async(&mut **client)
            .await?;
        Ok(())
    }

    pub async fn create_user(&self, name: &str) -> anyhow::Result<bool> {
        let conn = self.pool.get().await?;
        let res = conn.execute(
            "INSERT INTO users (name) VALUES ($1) ON CONFLICT (name) DO NOTHING",
            &[&name]
        ).await?;
        Ok(res > 0)
    }

    pub async fn user_exists(&self, name: &str) -> anyhow::Result<bool> {
        let conn = self.pool.get().await?;
        let row = conn.query_opt(
            "SELECT 1 FROM users WHERE name = $1",
            &[&name]
        ).await?;
        Ok(row.is_some())
    }
}