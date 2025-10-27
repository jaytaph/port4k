use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;
use crate::models::account::Account;
use crate::models::types::{Direction, RoomId};

pub async fn go(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    // 1. parse direction
    let Some(dir_str) = intent.args.get(0) else {
        ctx.output.system("Usage: go <direction>").await;
        return Ok(());
    };

    // 2. make sure player is logged in / in-world
    let account = match ctx.account() {
        Ok(acc) => acc,
        Err(_) => {
            ctx.output.system("You are not logged in.").await;
            return Ok(());
        }
    };

    let cur_view = match ctx.room_view() {
        Ok(v) => v,
        Err(_) => {
            ctx.output.system("You are nowhere. There's nowhere to go.").await;
            return Ok(());
        }
    };

    let Some(dir) = intent.direction else {
        ctx.output.system(format!("'{}' is not a valid direction.", dir_str)).await;
        return Ok(());
    };

    // 3. attempt move via world/nav API
    match try_move_player(ctx.clone(), &account, cur_view.blueprint.id, dir).await {
        Ok(_) => { /* we already output stuff inside try_move_player */ }
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

/// Attempt to move the player from `from_room_id` in direction `dir`.
/// On success, this function is responsible for producing ALL relevant output
/// (leave narration, enter narration, new room view, prompt).
async fn try_move_player(ctx: Arc<CmdCtx>, account: &Account, from_room_id: RoomId, dir: Direction) -> Result<(), MoveError> {
    // 1. find exit
    let rv = ctx.room_view().map_err(|e| MoveError::Internal(format!("failed to get room view: {}", e)))?;
    let exit = rv.exits.iter().find(|e| e.direction == dir && e.from_room_id == from_room_id);
    let Some(exit) = exit else {
        return Err(MoveError::NoSuchExit);
    };

    // 2. check visibility & locked status (game rules)
    if !exit.is_visible_to(account) {
        return Err(MoveError::NoSuchExit); // pretend it doesn't exist
    }
    if exit.is_locked() {
        return Err(MoveError::ExitLocked);
    }


    // 3. call on_leave hook on current room
    //    Hook is allowed to:
    //    - print text (ctx.out.line/system)
    //    - veto movement (return false / Err)
    //    - modify world state
    if let Err(e) = ctx.registry.services.room.exit_room(ctx.clone()).await {
        // Lua says no? We treat that as blocked.
        // You could also model this as a special LuaReturn::Blocked("msg").
        return Err(MoveError::Blocked(format!("You can't seem to leave: {}", e)));
    }

    // 4. actually move player in world state
    let c = ctx.registry.services.zone
        .generate_cursor(ctx.clone(), &account, exit.to_room_id)
        .await
        .map_err(|e| MoveError::Internal(format!("failed to generate cursor: {e}")))?
    ;
    ctx.registry.services.room
        .enter_room(ctx.clone(), &c)
        .await
        .map_err(|e| MoveError::Internal(format!("failed to enter room: {e}")))?
    ;

    Ok(())
}

#[derive(Debug)]
enum MoveError {
    NoSuchExit,
    ExitLocked,
    Blocked(String),    // e.g. "The blast door won't budge."
    Internal(String),   // db errors, logic errors, etc.
}
