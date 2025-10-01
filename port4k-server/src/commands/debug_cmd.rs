use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::state::session::WorldMode;
use anyhow::Result;

#[allow(unused)]
pub async fn debug(ctx: Arc<CmdCtx>, raw: &str) -> Result<String> {
    let rest = raw.strip_prefix("@debug").unwrap().trim();
    let sub = rest.split_whitespace().next().unwrap_or("");

    match sub {
        "where" => {
            let s = ctx.sess.read().unwrap();
            let user = s.name.as_ref().map(|u| u.0.as_str()).unwrap_or("<guest>");
            let msg = match &s.world {
                Some(WorldMode::Live { room_id }) => {
                    format!("[debug] user={user} world=Live room_id={}\n", room_id)
                }
                Some(WorldMode::Playtest { bp, room, .. }) => {
                    format!("[debug] user={user} world=Playtest {}:{}\n", bp, room)
                }
                None => format!("[debug] user={user} world=None\n"),
            };
            Ok(msg)
        }
        _ => Ok("Usage: @debug where\n".into()),
    }
}
