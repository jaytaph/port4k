use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::models::types::Direction;
use std::sync::Arc;

pub async fn go(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    // 1. parse direction
    let Some(dir) = intent.direction else {
        ctx.output.system("Usage: go <direction>").await;
        return Ok(());
    };

    if !ctx.is_logged_in() {
        ctx.output.system("You are not logged in.").await;
        return Ok(());
    }
    if !ctx.has_cursor() {
        ctx.output.system("You are nowhere. There's nowhere to go.").await;
        return Ok(());
    }

    // 3. attempt move via world/nav API
    match try_move_player(ctx.clone(), dir).await {
        Ok(_) => { /* All is ok */ }
        Err(MoveError::NoSuchExit) => {
            ctx.output.line("You can't go that way.").await;
        }
        Err(MoveError::ExitLocked) => {
            ctx.output.line("The way is locked.").await;
        }
        Err(MoveError::Blocked(msg)) => {
            ctx.output.line(msg).await;
        }
        Err(MoveError::Internal(e)) => {
            // log for ops, don't leak ugly internals to player
            tracing::error!(error=%e, "go: move failed");
            ctx.output.system("You try to move, but something goes wrong.").await;
        }
    }

    Ok(())
}

async fn try_move_player(ctx: Arc<CmdCtx>, dir: Direction) -> Result<(), MoveError> {
    let c = ctx
        .cursor()
        .map_err(|e| MoveError::Internal(format!("no cursor: {}", e)))?;

    // Find exit
    let rv = ctx
        .room_view()
        .map_err(|e| MoveError::Internal(format!("failed to get room view: {}", e)))?;
    let exit = rv
        .exits
        .iter()
        .find(|e| e.direction == dir && e.from_room_id == c.room_id);
    let Some(exit) = exit else {
        return Err(MoveError::NoSuchExit);
    };

    // Check if we are allowed / capabile of moving through exit
    if !exit.is_visible_to() {
        return Err(MoveError::NoSuchExit); // pretend it doesn't exist
    }
    if exit.is_locked() {
        return Err(MoveError::ExitLocked);
    }

    // Exit the room
    if let Err(e) = ctx.registry.services.room.exit_room(ctx.clone()).await {
        // Lua says no? We treat that as blocked.
        // You could also model this as a special LuaReturn::Blocked("msg").
        return Err(MoveError::Blocked(format!("You can't seem to leave: {}", e)));
    }

    // Create a new cursor with the new room
    let new_cursor = ctx
        .registry
        .services
        .room
        .create_cursor(c.realm_id, exit.to_room_id, c.account_id)
        .await
        .map_err(|e| MoveError::Internal(format!("failed to create cursor: {}", e)))?;
    ctx.sess.write().set_cursor(Some(new_cursor));

    let cursor = ctx
        .cursor()
        .map_err(|e| MoveError::Internal(format!("no cursor after move: {}", e)))?;

    // Move to new room
    ctx.registry
        .services
        .room
        .enter_room(ctx.clone(), &cursor)
        .await
        .map_err(|e| MoveError::Internal(format!("failed to enter room: {e}")))?;
    Ok(())
}

#[derive(Debug)]
enum MoveError {
    NoSuchExit,
    ExitLocked,
    Blocked(String),  // e.g. "The blast door won't budge."
    Internal(String), // db errors, logic errors, etc.
}
