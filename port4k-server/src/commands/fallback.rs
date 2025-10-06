use std::sync::Arc;
use std::time::Duration;
use crate::commands::{CmdCtx, CommandResult};
use tokio::sync::oneshot;
use tokio::time::timeout;
use crate::commands::CommandResult::Failure;
use crate::models::zone::ZoneKind;
use crate::error::{AppError, AppResult};
use crate::input::parser::Intent;
use crate::lua::LuaJob;

const LUA_CMD_TIMEOUT: Duration = Duration::from_secs(2);

/// Handles non-matched commands:
/// - If in Playtest, forwards to Lua on_command
/// - Otherwise prints "Unknown command"
pub async fn fallback(ctx: Arc<CmdCtx>, intent: Intent) -> AppResult<CommandResult> {
    let (zone_id, zone_kind, bp_id, room_id, account) = {
        let account = ctx.account()?;

        let c = match s.cursor.as_ref() {
            Some(c) => c,
            None => return Ok(Failure("You must `enter` a world first.\n".into())),
        };

        (c.zone.id, c.zone_kind.clone(), c.bp.id, c.room.room.id, account)
    };

    // @TODO: why are we only doing lua scripting in ZoneKind::Test?
    let scripting_enabled = matches!(zone_kind, ZoneKind::Test { .. });
    if !scripting_enabled {
        return Ok(Failure("Unknown command. Try `help`.\n".into()));
    }

    let (tx, rx) = oneshot::channel();
    ctx.state.lua_tx.send(LuaJob::OnCommand {
        zone_id,
        zone_kind,
        bp_id,
        room_id,
        account_id: account.id,
        intent,
        reply: tx,
    }).await.map_err(|_| AppError::Lua("could not send command"))?;

    match timeout(LUA_CMD_TIMEOUT, rx).await {
        Err(_) => Ok(Failure("The room doesn't react (script timed out)\n".into())),
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Ok(Failure("The room doesn't react (script error)\n".into())),
    }
}
