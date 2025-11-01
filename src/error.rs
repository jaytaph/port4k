use crate::db::error::DbError;
use crate::lua::LuaJob;
use crate::models::types::{ObjectId, RoomId};
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

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

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("invalid direction: {0}")]
    InvalidDirection(String),

    #[error("validation failed: {field}: {message}")]
    Validation { field: &'static str, message: String },

    #[error(transparent)]
    Send(#[from] Box<SendError<LuaJob>>),

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

    #[error("blueprint or room not found")]
    RoomNotFound,

    #[error("script error: {0}")]
    Script(String),

    #[error("script lua error: {0}")]
    ScriptLua(#[from] mlua::Error),

    #[error("login error: {0}")]
    LoginError(String),
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


#[derive(Debug, Error)]
pub enum LoginError {
    #[error("user not found")]
    UserNotFound,
    #[error("invalid password")]
    InvalidPassword,
    #[error("account locked")]
    AccountLocked,
    #[error("too many attempts")]
    TooManyAttempts,
    #[error("internal error: {0}")]
    InternalError(String)
}