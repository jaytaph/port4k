use thiserror::Error;
use crate::db::DbError;
use crate::models::types::{ObjectId, RoomId};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("entity not found: {entity}")]
    NotFound { entity: &'static str },

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("business rule: {0}")]
    RuleViolated(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error(transparent)]
    Database(#[from] DbError),

    #[error(transparent)]
    PasswordHash(#[from] password_hash::Error),
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),

    #[error("usage: {0}")]
    Usage(String),

    #[error("permission denied")]
    PermissionDenied,

    #[error(transparent)]
    Service(#[from] ServiceError),
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not logged in")]
    NotLoggedIn,

    #[error("Not found")]
    NotFound,

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

    #[error("You are not in a world")]
    NoCursor,

    #[error("validation failed: {field}: {message}")]
    Validation { field: &'static str, message: &'static str },

    #[error("Telnet error: {0}")]
    Telnet(String),

    #[error("insufficient quantity for object {obj_id} in room {room_id}: have {have}, need {need}")]
    InsufficientQuantity { room_id: RoomId, obj_id: ObjectId, have: i32, need: i32 },

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict")]
    Conflict,

    #[error(transparent)]
    Command(CommandError),

    #[error(transparent)]
    Service(#[from] ServiceError),
}

impl From<DbError> for AppError {
    fn from(e: DbError) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<CommandError> for AppError {
    fn from(e: CommandError) -> Self {
        match &e {
            CommandError::UnknownCommand(_) | CommandError::Usage(_) =>
                AppError::BadRequest(e.to_string()),
            CommandError::PermissionDenied =>
                AppError::BadRequest("permission denied".into()),
            CommandError::Service(ServiceError::NotFound{..}) =>
                AppError::NotFound,
            CommandError::Service(ServiceError::Conflict(_)) =>
                AppError::Conflict,
            _ => AppError::Command(e),
        }
    }
}

