use crate::commands::CmdCtx;
use crate::state::session::WorldMode;
use anyhow::Result;
use tokio::sync::oneshot;

/// Handles non-matched commands:
/// - If in Playtest, forwards to Lua on_command
/// - Otherwise prints "Unknown command"
pub async fn fallback(ctx: &CmdCtx<'_>, verb: &str, args: Vec<String>) -> Result<String> {
    let (bp, room, user) = {
        let s = ctx.sess.lock().await;
        match (&s.world, &s.name) {
            (Some(WorldMode::Playtest { bp, room, .. }), Some(u)) => {
                (bp.clone(), room.clone(), u.0.clone())
            }
            _ => return Ok("Unknown command. Try `help`.\n".into()),
        }
    };

    let (tx, rx) = oneshot::channel();
    ctx.lua_tx
        .send(crate::lua::LuaJob::OnCommandPlaytest {
            db: ctx.registry.db.clone(),
            bp,
            room,
            account: user,
            verb: verb.to_string(),
            args,
            reply: tx,
        })
        .await?;

    match rx.await?? {
        Some(out) if !out.trim().is_empty() => Ok(out),
        _ => Ok("Unknown command. Try `help`.\n".into()),
    }
}
