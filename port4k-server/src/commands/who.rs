use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::services::CommandResult;
use crate::success;

pub async fn who(ctx: Arc<CmdCtx>) -> CommandResult<CommandOutput> {
    let list = ctx.state.registry.who().await;
    Ok(if list.is_empty() {
        success!("No one is online.\n")
    } else {
        success!(format!("Online ({}): {}\n", list.len(), list.join(", ")))
    })
}
