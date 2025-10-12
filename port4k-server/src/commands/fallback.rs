use std::sync::Arc;
use std::time::Duration;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use tokio::sync::oneshot;
use tokio::time::timeout;
use crate::models::zone::ZoneKind;
use crate::{failure, success};
use crate::input::parser::Intent;
use crate::lua::LuaJob;

#[allow(unused)]
const LUA_CMD_TIMEOUT: Duration = Duration::from_secs(2);

/// Handles non-matched commands:
/// - If in Playtest, forwards to Lua on_command
/// - Otherwise prints "Unknown command"
pub async fn fallback(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let account = ctx.account()?;
    let cursor = ctx.cursor()?;

    // @TODO: why are we only doing lua scripting in ZoneKind::Test?
    let scripting_enabled = matches!(cursor.zone_ctx.kind, ZoneKind::Test { .. });
    if !scripting_enabled {
        return Ok(failure!("Unknown command. Try `help`.\n"));
    }

    let (tx, rx) = oneshot::channel();
    ctx.lua_tx.send(LuaJob::OnCommand {
        cursor,
        account,
        intent,
        reply: tx,
    }).await?;

    match timeout(LUA_CMD_TIMEOUT, rx).await {
        Err(_) => Ok(failure!("The room doesn't react (script timed out)\n")),
        Ok(Ok(result)) => Ok(success!(result)),
        Ok(Err(_)) => Ok(failure!("The room doesn't react (script error)\n")),
    }
}
