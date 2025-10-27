use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::lua::{LuaJob, LUA_CMD_TIMEOUT};
use crate::models::zone::ZoneKind;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::timeout;

pub async fn fallback(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    // let account = ctx.account()?;
    let cursor = ctx.cursor()?;
    let account = ctx.account()?;

    // @TODO: why are we only doing lua scripting in ZoneKind::Test?
    let scripting_enabled = matches!(cursor.zone_ctx.kind, ZoneKind::Test { .. });
    if !scripting_enabled {
        ctx.output.system("Unknown command. Try `help`.").await;
        return Ok(());
    }

    let output_handle = ctx.output.clone();

    let (tx, rx) = oneshot::channel();
    ctx.lua_tx
        .send(LuaJob::OnCommand {
            output_handle,
            account,
            cursor: Box::new(cursor),
            intent: Box::new(intent),
            reply: tx,
        })
        .await
        .map_err(Box::new)?;

    match timeout(LUA_CMD_TIMEOUT, rx).await {
        Err(_) => {
            ctx.output.system("The room doesn't react (script timed out)").await;
        }
        Ok(Ok(_)) => {
            // Not handled by lua
        }
        Ok(Err(_)) => {
            ctx.output.system("The room doesn't react (script error)").await;
        }
    }

    Ok(())
}
