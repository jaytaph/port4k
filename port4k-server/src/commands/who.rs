use crate::commands::{CmdCtx, CommandResult};
use std::sync::Arc;

pub async fn who(ctx: Arc<CmdCtx>) -> CommandResult {
    let list = ctx.registry.who().await;
    if list.is_empty() {
        ctx.output.system("No one is online.").await;
    } else {
        ctx.output
            .system(format!("Online ({}): {}\n", list.len(), list.join(", ")))
            .await;
    };

    Ok(())
}
