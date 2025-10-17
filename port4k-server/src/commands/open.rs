use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;
use tokio::sync::oneshot;
use crate::error::DomainError;
use crate::lua::LuaJob;

pub async fn open(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    let rv = ctx.room_view()?;

    let Some(noun) = intent.direct.as_ref() else {
        out.append("Open what?\n");
        out.failure();
        return Ok(out);
    };

    let mut handled = false;

    // Check if we are opening an object
    if let Some(obj) = rv.object_by_noun(&noun.head) {
        // Do we have a script attached? run that first
        if let Some(_) = obj.use_lua.as_ref() {
            let (tx, rx) = oneshot::channel();

            ctx.lua_tx.send(LuaJob::OnObject {
                account: ctx.account()?,
                cursor: Box::new(ctx.cursor()?),
                intent: Box::new(intent.clone()),
                reply: tx,
            }).await.map_err(|_| DomainError::InternalError("Failed to send Lua job".into()))?;

            match rx.await.map_err(|_| DomainError::InternalError("Lua script channel closed".into()))? {
                Some(result) => {
                    for msg in result.data {
                        out.append(&msg);
                    }
                    if result.ok {
                        out.success();
                    } else {
                        out.failure();
                    }
                    handled = true;
                },
                None => {
                    // Not handled by Lua
                }
            }
        }
    }

    // Check if we want to open a direction


    if !handled {
        // Nothing has handled the open command
        out.append("You try to open it, but nothing happens.\n");
        out.failure();
    }

    Ok(out)
}
