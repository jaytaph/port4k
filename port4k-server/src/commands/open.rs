use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;
use tokio::sync::oneshot;
use crate::error::DomainError;
use crate::lua::{LuaJob, LuaResult};

pub async fn open(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    let rv = ctx.room_view()?;

    let Some(noun) = intent.direct.as_ref() else {
        ctx.output.system("Open what?").await;
        return Ok(());
    };

    let mut handled = false;

    // Check if we are opening an object
    if let Some(obj) = rv.object_by_noun(&noun.head) {
        // Do we have a script attached? run that first
        if let Some(_) = obj.on_use.as_ref() {
            let (tx, rx) = oneshot::channel();

            let output_handle = ctx.output.clone();

            ctx.lua_tx.send(LuaJob::OnObject {
                output_handle,
                account: ctx.account()?,
                cursor: Box::new(ctx.cursor()?),
                intent: Box::new(intent.clone()),
                obj: Box::new(obj.clone()),
                reply: tx,
            }).await.map_err(|_| DomainError::InternalError("Failed to send Lua job".into()))?;

            match rx.await.map_err(|_| DomainError::InternalError("Lua script channel closed".into()))? {
                LuaResult::Success(v) => {
                    // Only if returned "true" then we consider it handled
                    if v.is_boolean() && v.as_boolean().unwrap_or(false) {
                        handled = true;
                    } else {
                        handled = false;
                    }
                },
                LuaResult::Failed(msg) => ctx.output.system(format!("on_object script returned an error: {}", msg)).await,
            }
        }
    }

    // Check if we want to open a direction

    if !handled {
        // Nothing has handled the open command
        ctx.output.line("You try to open it, but nothing happens.").await;
    }

    Ok(())
}
