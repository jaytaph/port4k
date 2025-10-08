mod auth;
mod account;
mod blueprint;
mod room;

pub use auth::AuthService;
pub use account::AccountService;
pub use blueprint::BlueprintService;
pub use room::RoomService;

use crate::error::ServiceError;

pub type ServiceResult<T> = Result<T, ServiceError>;
