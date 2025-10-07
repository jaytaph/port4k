mod auth;
mod account;
mod blueprint;

pub use auth::AuthService;
pub use account::AccountService;
pub use blueprint::BlueprintService;
use crate::error::{CommandError, ServiceError};

pub type ServiceResult<T> = Result<T, ServiceError>;
pub type CommandResult<T> = Result<T, CommandError>;
