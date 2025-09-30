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
mod migrations;
mod pool;
pub mod types;

pub mod accounts;
pub mod blueprint;
pub mod characters;
pub mod loot;
pub mod rooms;

mod repo {
    pub mod object;
}
