use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::state::session::WorldMode;
use anyhow::Result;
use tokio::sync::oneshot;
use crate::commands::CommandResult::{Failure, Success};

/// Handles non-matched commands:
/// - If in Playtest, forwards to Lua on_command
/// - Otherwise prints "Unknown command"
pub async fn fallback(ctx: Arc<CmdCtx>, verb: &str, args: Vec<String>) -> Result<CommandResult> {
    let (bp, room, account) = {
        let s = ctx.sess.read().map_err(|_| anyhow::anyhow!("Session lock poisoned"))?;
        match (&s.world, &s.account) {
            (Some(WorldMode::Playtest { bp, room, .. }), Some(a)) => (bp.clone(), room.clone(), a.clone()),
            _ => return Ok(Failure("Unknown command. Try `help`.\n".into())),
        }
    };

    let (tx, rx) = oneshot::channel();
    ctx.lua_tx
        .send(crate::lua::LuaJob::OnCommandPlaytest {
            db: ctx.registry.db.clone(),
            bp,
            room,
            account,
            verb: verb.to_string(),
            args,
            reply: tx,
        })
        .await?;

    match rx.await?? {
        Some(out) if !out.trim().is_empty() => Ok(Success(out)),
        _ => Ok(Failure("Unknown command. Try `help`.\n".into())),
    }
}
