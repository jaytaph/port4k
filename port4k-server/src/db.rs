use deadpool_postgres::Pool;
use crate::db::error::DbError;

// keep public API surface by re-exporting submodules
mod migrations;
mod pool;

pub mod accounts;
pub mod blueprint;
pub mod characters;
pub mod loot;
pub mod rooms;

pub mod repo;
pub mod error;

pub type DbResult<T> = Result<T, DbError>;

#[derive(Clone, Debug)]
pub struct Db {
    pub(crate) pool: Pool,
}

impl Db {
    pub async fn get_client(&self) -> DbResult<deadpool_postgres::Client> {
        Ok(self.pool.get().await?)
    }
}