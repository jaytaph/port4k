use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::{ConnState, WorldMode};
use anyhow::Result;
use crate::commands::CommandResult::{Failure, Success};

pub async fn look(ctx: Arc<CmdCtx>, _intent: Intent) -> Result<CommandResult> {
    let world = {
        let s = ctx.sess.read().unwrap();
        if s.state != ConnState::LoggedIn {
            return Ok(Failure("You must `login` first.\n".into()));
        }

        s.world.clone()
    };

    match &world {
        Some(WorldMode::Live { room_id }) => {
            let view = ctx.registry.db.room_view(*room_id).await?;
            Ok(Success(view))
        }
        Some(WorldMode::Playtest { bp, room, .. }) => match ctx.registry.repos.room.bp_room_view(bp, room, 80).await? {
            Some(view) => Ok(Success(view)),
            None => Ok(Failure("[playtest] This room does not exist.\n".into())),
        },
        None => Ok(Failure("You are nowhere.\n".into())),
    }
}
