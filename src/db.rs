use crate::db::error::DbError;
use deadpool_postgres::Pool;
use tokio_postgres::Row;

// keep public API surface by re-exporting submodules
mod migrations;
mod pool;

pub mod blueprint;
pub mod characters;
pub mod loot;

pub mod error;
pub mod repo;

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

fn map_row<T, E>(
    row: &tokio_postgres::Row,
    f: impl FnOnce(&tokio_postgres::Row) -> Result<T, E>,
    ctx: &str
)  -> Result<T, E>
where
    E: std::fmt::Display,
{
    match f(row) {
        Ok(v) => Ok(v),
        Err(e) => {
            tracing::error!("row deserialization failed in {ctx}: {e}");
            Err(e)
        }
    }
}

pub fn map_row_opt<T, F>(row_opt: Option<Row>, f: F, ctx: &str) -> DbResult<Option<T>>
where
    F: FnOnce(&Row) -> DbResult<T>,
{
    match row_opt {
        Some(row) => {
            match f(&row) {
                Ok(v) => Ok(Some(v)),
                Err(e) => {
                    tracing::error!(error = %e, context = %ctx, "row mapping failed");
                    Err(e)
                }
            }
        }
        None => Ok(None),
    }
}