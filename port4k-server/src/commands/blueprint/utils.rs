use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::error::{AppError, AppResult};

/// Get the current logged-in account name as String.
pub fn current_owner(ctx: Arc<CmdCtx>) -> AppResult<String> {
    if ! ctx.is_logged_in() {
        return Err(AppError::NotLoggedIn);
    }

    Ok(ctx.account()?.username)
}