use thiserror::Error;
use crate::db::DbError;
use crate::models::types::{ObjectId, RoomId};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not logged in")]
    NotLoggedIn,

    #[error("Global error: {0}")]
    Custom(String),

    #[error("Lua error: {0}")]
    Lua(&'static str),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Db(String),

    #[error("Invalid arguments: {0}")]
    Args(&'static str),

    #[error("validation failed: {field}: {message}")]
    Validation { field: &'static str, message: &'static str },

    #[error("Telnet error: {0}")]
    Telnet(String),

    #[error("Internal error")]
    Internal(#[from] InfraError),

    #[error("insufficient quantity for object {obj_id} in room {room_id}: have {have}, need {need}")]
    InsufficientQuantity { room_id: RoomId, obj_id: ObjectId, have: i32, need: i32 },
}

#[derive(Debug, Error)]
pub enum InfraError {
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<DbError> for AppError {
    fn from(e: DbError) -> Self {
        AppError::Internal(InfraError::Db(e))
    }
}

pub type AppResult<T> = Result<T, AppError>;