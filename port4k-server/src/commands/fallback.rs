use std::sync::Arc;
use std::time::Duration;
use crate::commands::{CmdCtx, CommandOutput};
use tokio::sync::oneshot;
use tokio::time::timeout;
use crate::models::zone::ZoneKind;
use crate::error::AppError;
use crate::input::parser::Intent;
use crate::lua::LuaJob;
use crate::services::CommandResult;

const LUA_CMD_TIMEOUT: Duration = Duration::from_secs(2);

/// Handles non-matched commands:
/// - If in Playtest, forwards to Lua on_command
/// - Otherwise prints "Unknown command"
pub async fn fallback(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let (cursor, account_id) = {
        let account_id = ctx.account_id()?;
        let cursor = ctx.cursor()?;

        (cursor, account_id)
    };

    // @TODO: why are we only doing lua scripting in ZoneKind::Test?
    let scripting_enabled = matches!(cursor.zone_kind, ZoneKind::Test { .. });
    if !scripting_enabled {
        return Ok(Failure("Unknown command. Try `help`.\n".into()));
    }

    let (tx, rx) = oneshot::channel();
    ctx.state.lua_tx.send(LuaJob::OnCommand {
        cursor,
        account_id,
        intent,
        reply: tx,
    }).await.map_err(|_| AppError::Lua("could not send command"))?;

    match timeout(LUA_CMD_TIMEOUT, rx).await {
        Err(_) => Ok(Failure("The room doesn't react (script timed out)\n".into())),
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Ok(Failure("The room doesn't react (script error)\n".into())),
    }
}
