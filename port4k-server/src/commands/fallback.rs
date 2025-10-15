use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::lua::LuaJob;
use crate::models::zone::ZoneKind;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[allow(unused)]
const LUA_CMD_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn fallback(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    let account = ctx.account()?;
    let cursor = ctx.cursor()?;

    // @TODO: why are we only doing lua scripting in ZoneKind::Test?
    let scripting_enabled = matches!(cursor.zone_ctx.kind, ZoneKind::Test { .. });
    if !scripting_enabled {
        out.append("Unknown command. Try `help`.\n");
        out.failure();
        return Ok(out);
    }

    let (tx, rx) = oneshot::channel();
    ctx.lua_tx
        .send(LuaJob::OnCommand {
            cursor,
            account,
            intent,
            reply: tx,
        })
        .await?;

    match timeout(LUA_CMD_TIMEOUT, rx).await {
        Err(_) => {
            out.append("The room doesn't react (script timed out)\n");
            out.failure();
        }
        Ok(Ok(_)) => {
            out.append("result from lua\n");
            out.success();
        }
        Ok(Err(_)) => {
            out.append("The room doesn't react (script error)\n");
            out.failure();
        }
    }
    Ok(out)
}
