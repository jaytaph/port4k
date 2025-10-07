use deadpool_postgres::{BuildError, Pool, PoolError};
use thiserror::Error;

// keep public API surface by re-exporting submodules
mod migrations;
mod pool;

pub mod accounts;
pub mod blueprint;
pub mod characters;
pub mod loot;
pub mod rooms;

pub mod repo;


#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    Pool(#[from] PoolError),

    #[error(transparent)]
    Pg(#[from] tokio_postgres::Error),
    #[error(transparent)]
    Migrate(#[from] refinery::Error),

    #[error(transparent)]
    Build(#[from] BuildError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
    
    #[error("row decode error: {0}")]
    Decode(&'static str),
}

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