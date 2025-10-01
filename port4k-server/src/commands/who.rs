use std::sync::Arc;
use crate::commands::CmdCtx;
use anyhow::Result;

pub async fn who(ctx: Arc<CmdCtx>) -> Result<String> {
    let list = ctx.registry.who().await;
    Ok(if list.is_empty() {
        "No one is online.\n".into()
    } else {
        format!("Online ({}): {}\n", list.len(), list.join(", "))
    })
}
