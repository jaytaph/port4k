use deadpool_postgres::{BuildError, PoolError};
use thiserror::Error;

// DbError is the lowest level error type, wrapping errors from the database layer. It does not wrap
// any higher level errors.
#[derive(Debug, Error)]
pub enum DbError {
    /// Record not found
    #[error("not found")]
    NotFound,

    /// Unique constraint violation
    #[error("unique violation")]
    UniqueViolation,

    /// Foreign key constraint violation
    #[error("foreign key violation")]
    ForeignKey,

    /// Timeout error
    #[error("timeout")]
    Timeout,

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
    Decode(String),

    #[error("input error: {0}")]
    Validation(String),
}
