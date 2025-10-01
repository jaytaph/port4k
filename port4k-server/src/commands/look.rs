use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::input::parser::Intent;
use crate::state::session::{ConnState, WorldMode};
use anyhow::Result;

pub async fn look(ctx: Arc<CmdCtx>, _intent: Intent) -> Result<String> {
    let world = {
        let s = ctx.sess.read().unwrap();
        if s.state != ConnState::LoggedIn {
            return Ok("You must `login` first.\r\n".into());
        }

        s.world.clone()
    };

    match &world {
        Some(WorldMode::Live { room_id }) => {
            let view = ctx.registry.db.room_view(*room_id).await?;
            Ok(view)
        }
        Some(WorldMode::Playtest { bp, room, .. }) => match ctx.registry.db.bp_room_view(bp, room, 80).await? {
            Some(view) => Ok(view),
            None => Ok("[playtest] This room does not exist.\r\n".into()),
        },
        None => Ok("You are nowhere.\r\n".into()),
    }
}
