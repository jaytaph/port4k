mod auth;
mod account;
mod blueprint;
mod room;
mod cursor;
mod navigator;
mod error;

pub use auth::AuthService;
pub use account::AccountService;
pub use blueprint::BlueprintService;
pub use room::RoomService;
pub use cursor::CursorService;
pub use navigator::NavigatorService;

pub use error::ServiceError;

// pub type ServiceResult<T> = Result<T, ServiceError>;
