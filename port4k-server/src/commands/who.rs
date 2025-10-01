use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use anyhow::Result;
use crate::commands::CommandResult::Success;

pub async fn who(ctx: Arc<CmdCtx>) -> Result<CommandResult> {
    let list = ctx.registry.who().await;
    Ok(if list.is_empty() {
        Success("No one is online.\n".into())
    } else {
        Success(format!("Online ({}): {}\n", list.len(), list.join(", ")))
    })
}
