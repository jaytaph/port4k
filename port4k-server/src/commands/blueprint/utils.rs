use std::sync::Arc;
use crate::commands::CmdCtx;
use anyhow::{anyhow, Result};

/// Get the current logged-in account name as String.
pub fn current_owner(ctx: Arc<CmdCtx>) -> Result<String> {
    ctx.sess
        .read()
        .unwrap()
        .account
        .as_ref()
        .map(|a| a.username.clone())
        .ok_or_else(|| anyhow!("login required"))
}