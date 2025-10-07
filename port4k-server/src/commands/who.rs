use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::error::AppResult;

pub async fn who(ctx: Arc<CmdCtx>) -> AppResult<CommandOutput> {
    let list = ctx.state.registry.who().await;
    Ok(if list.is_empty() {
        CommandOutput { is_error: false, message: "No one is online.\n".into() }
    } else {
        CommandOutput { is_error: false, message: format!("Online ({}): {}\n", list.len(), list.join(", ")) }
    })
}
