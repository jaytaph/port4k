use deadpool_postgres::Pool;

#[derive(Clone, Debug)]
pub struct Db {
    pub(crate) pool: Pool,
}

impl Db {
    #[allow(unused)]
    pub async fn get_client(&self) -> anyhow::Result<deadpool_postgres::Client> {
        Ok(self.pool.get().await?)
    }
}

// keep public API surface by re-exporting submodules
pub mod types;
mod pool;
mod migrations;

pub mod accounts;
pub mod characters;
pub mod rooms;
pub mod loot;
pub mod blueprint;
