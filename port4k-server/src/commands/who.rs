use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::commands::CommandResult::Success;
use crate::error::AppResult;

pub async fn who(ctx: Arc<CmdCtx>) -> AppResult<CommandResult> {
    let list = ctx.state.registry.who().await;
    Ok(if list.is_empty() {
        Success("No one is online.\n".into())
    } else {
        Success(format!("Online ({}): {}\n", list.len(), list.join(", ")))
    })
}
