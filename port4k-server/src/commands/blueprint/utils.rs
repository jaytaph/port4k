use std::sync::Arc;
use crate::commands::CmdCtx;
use anyhow::{anyhow, Result};

/// Get the current logged-in account name as String.
pub fn current_owner(ctx: Arc<CmdCtx>) -> Result<String> {
    ctx.sess
        .read()
        .unwrap()
        .name
        .as_ref()
        .map(|u| u.0.clone())
        .ok_or_else(|| anyhow!("login required"))
}