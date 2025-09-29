use crate::commands::CmdCtx;
use crate::state::session::{ConnState, WorldMode};
use anyhow::Result;

pub async fn playtest(ctx: &CmdCtx<'_>, raw: &str) -> Result<String> {
    let rest = raw.strip_prefix("@playtest").unwrap().trim();

    if rest.eq_ignore_ascii_case("stop") {
        let mut s = ctx.sess.lock().await;
        match &mut s.world {
            Some(WorldMode::Playtest { prev_room_id, .. }) => {
                let room_id = prev_room_id
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("no previous location"))?;
                s.world = Some(WorldMode::Live { room_id });
                drop(s); // release lock before DB call
                let view = ctx.registry.db.room_view(room_id).await?;
                Ok(format!("[playtest] exited.\n{view}"))
            }
            _ => Ok("[playtest] you are not in playtest.\n".into()),
        }
    } else {
        // Enter playtest with key in `rest`
        let bp = rest;
        let entry = ctx
            .registry
            .db
            .bp_entry(bp)
            .await?
            .ok_or_else(|| anyhow::anyhow!("blueprint has no entry room"))?;

        let mut s = ctx.sess.lock().await;
        if s.state != ConnState::LoggedIn {
            return Ok("Login required.\n".into());
        }
        let prev = match &s.world {
            Some(WorldMode::Live { room_id }) => Some(*room_id),
            _ => None,
        };
        s.world = Some(WorldMode::Playtest {
            bp: bp.to_string(),
            room: entry.clone(),
            prev_room_id: prev,
        });
        drop(s); // release lock before DB call

        let view = ctx
            .registry
            .db
            .bp_room_view(bp, &entry)
            .await?
            .unwrap_or_else(|| "[playtest] empty room\n".into());

        Ok(format!(
            "[playtest] entered `{}` at `{}`.\n{}",
            bp, entry, view
        ))
    }
}
