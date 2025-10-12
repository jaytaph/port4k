use thiserror::Error;
use crate::db::error::DbError;
use crate::error::DomainError;

/// ServiceError represents errors that can occur in the service layer of the application.
/// It encapsulates various error scenarios including not found, invalid input, business rule violations,
/// database errors, password hashing errors, validation errors, I/O errors, and internal errors.
///
/// Wraps lower-level errors to provide a consistent error handling mechanism across the service layer.
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
    PasswordHash(#[from] password_hash::Error),

    #[error("validation failed: {field}: {message}")]
    Validation { field: &'static str, message: String },

    // #[error(transparent)]
    // Io(#[from] std::io::Error),

    #[error("Internal error")]
    Internal(#[from] anyhow::Error),

    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Database(#[from] DbError),
}