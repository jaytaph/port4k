use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::lua::{LUA_CMD_TIMEOUT, LuaJob, LuaResult};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::timeout;

pub async fn fallback(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    // let account = ctx.account()?;
    let cursor = ctx.cursor()?;
    let account_id = ctx.account_id()?;
    let output_handle = ctx.output.clone();

    // We send the onCommand job. The job will check if we actually have a onCommand script in the room
    let (tx, rx) = oneshot::channel();
    ctx.lua_tx
        .send(LuaJob::OnCommand {
            output_handle,
            account_id,
            cursor: Box::new(cursor),
            intent: Box::new(intent),
            reply: tx,
        })
        .await
        .map_err(Box::new)?;

    match timeout(LUA_CMD_TIMEOUT, rx).await {
        Ok(Ok(lua_result)) => match lua_result {
            LuaResult::Failed(msg) => {
                let s = format!("{{c:yellow:bright_red}}Lua script failure: {msg}{{c}}");
                ctx.output.system(s).await;
                return Ok(());
            }
            LuaResult::Success(v) => {
                if v.as_boolean().unwrap_or(false) {
                    // Script handled the command
                    return Ok(());
                }

                // Script did not handle the command, for now, we just return "unknown command"
                let s = "{c:bright_red}Unknown command specified.{c}";
                ctx.output.system(s).await;
            }
        },
        Ok(Err(e)) => {
            let s = format!("{{c:yellow:bright_red}}Internal system error: {e}{{c}}");
            ctx.output.system(s).await;
        }
        Err(_elapsed) => {
            let s = "{c:yellow:bright_red}The room doesn't react (script timed out){c}";
            ctx.output.system(s).await;
        }
    }

    Ok(())
}
