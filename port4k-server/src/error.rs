use crate::db::error::DbError;
use crate::models::types::{ObjectId, RoomId};
use thiserror::Error;

pub type AppResult<T> = Result<T, DomainError>;

#[derive(Debug, Error)]
pub enum DomainError {
    /// Room is locked
    #[error("locked exit: {0}")]
    LockedExit(String),

    /// Insufficient quantity to perform action
    #[error("insufficient quantity: have {have}, need {need}")]
    InsufficientQuantity {
        room_id: RoomId,
        obj_id: ObjectId,
        have: i32,
        need: i32,
    },

    /// No current room
    #[error("no current room")]
    NoCurrentRoom,

    /// Permission is denied
    #[error("permission denied")]
    PermissionDenied,

    /// Some precondition failed
    #[error("precondition failed: {0}")]
    PreconditionFailed(&'static str),

    #[error(transparent)]
    Db(#[from] DbError),

    #[error(transparent)]
    Infra(#[from] InfraError),

    #[error("not logged in")]
    NotLoggedIn,

    #[error("Not found")]
    NotFound,

    #[error("invalid direction: {0}")]
    InvalidDirection(String),

    #[error("validation failed: {field}: {message}")]
    Validation { field: &'static str, message: String },

    #[error(transparent)]
    Lua(#[from] mlua::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    Password(#[from] password_hash::Error),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("internal error: {0}")]
    InternalError(String),
}

#[derive(Debug, Error)]
pub enum ConfigErrorKind {
    #[error("failed to read file: {0}")]
    Read(std::io::Error),

    #[error("failed to parse file: {0}")]
    Parse(toml::de::Error),

    #[error("missing environment variable: {0}")]
    MissingEnv(String),

    #[error("invalid environment variable {0}: {1}")]
    InvalidEnv(String, String),
}

#[derive(Debug, Error)]
pub enum InfraError {
    #[error(transparent)]
    Db(#[from] DbError),

    #[error("invalid configuration in {path}: {source}")]
    Config {
        path: std::path::PathBuf,
        #[source]
        source: ConfigErrorKind,
    },

    #[error("missing env var: {0}")]
    MissingEnv(String),

    #[error("network issue: {0}")]
    Net(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

// #[derive(Debug, Error)]
// pub enum AppError {
//     #[error("Not logged in")]
//     NotLoggedIn,
//
//     #[error("Not found")]
//     NotFound,
//
//     #[error("Global error: {0}")]
//     Custom(String),
//
//     #[error("Configuration error: {0}")]
//     Config(String),
//
//     #[error("Invalid arguments: {0}")]
//     Args(&'static str),
//
//     #[error("You are not in a world")]
//     NoCursor,
//
//     #[error("validation failed: {field}: {message}")]
//     Validation { field: &'static str, message: String },
//
//     #[error("Telnet error: {0}")]
//     Telnet(String),
//
//     #[error("insufficient quantity for object {obj_id} in room {room_id}: have {have}, need {need}")]
//     InsufficientQuantity { room_id: RoomId, obj_id: ObjectId, have: i32, need: i32 },
//
//     #[error("Bad request: {0}")]
//     BadRequest(String),
//
//     #[error("Conflict")]
//     Conflict,
//
//     #[error(transparent)]
//     Command(CommandError),
//
//     #[error(transparent)]
//     Service(#[from] ServiceError),
//
//     #[error(transparent)]
//     Postgres(#[from] tokio_postgres::Error),
//
//     #[error(transparent)]
//     Db(#[from] DbError),
//
//     #[error(transparent)]
//     Json(#[from] serde_json::Error),
//
//     #[error(transparent)]
//     Lua(#[from] mlua::Error),
//
//     #[error(transparent)]
//     Io(#[from] std::io::Error),
// }

// impl From<CommandError> for AppError {
//     fn from(e: CommandError) -> Self {
//         match &e {
//             CommandError::UnknownCommand(_) | CommandError::Usage(_) =>
//                 AppError::BadRequest(e.to_string()),
//             CommandError::PermissionDenied =>
//                 AppError::BadRequest("permission denied".into()),
//             CommandError::Service(ServiceError::NotFound{..}) =>
//                 AppError::NotFound,
//             CommandError::Service(ServiceError::Conflict(_)) =>
//                 AppError::Conflict,
//             _ => AppError::Command(e),
//         }
//     }
// }
//
