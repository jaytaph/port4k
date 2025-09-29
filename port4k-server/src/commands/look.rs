use crate::commands::CmdCtx;
use crate::state::session::{ConnState, WorldMode};
use anyhow::Result;

pub async fn look(ctx: &CmdCtx<'_>) -> Result<String> {
    let s = ctx.sess.lock().await;
    if s.state != ConnState::LoggedIn {
        return Ok("You must `login` first.\n".into());
    }
    match &s.world {
        Some(WorldMode::Live { room_id }) => {
            let view = ctx.registry.db.room_view(*room_id).await?;
            Ok(view)
        }
        Some(WorldMode::Playtest { bp, room, .. }) => {
            match ctx.registry.db.bp_room_view(bp, room).await? {
                Some(view) => Ok(view),
                None => Ok("[playtest] This room does not exist.\n".into()),
            }
        }
        None => Ok("You are nowhere.\n".into()),
    }
}
