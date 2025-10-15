use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};

pub async fn who(ctx: Arc<CmdCtx>) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    let list = ctx.registry.who().await;
    if list.is_empty() {
        out.append("No one is online.\n");
    } else {
        out.append(format!("Online ({}): {}\n", list.len(), list.join(", ")).as_str());
    };

    out.success();
    Ok(out)
}
