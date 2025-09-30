use crate::commands::CmdCtx;
use crate::state::session::WorldMode;
use anyhow::Result;

pub async fn debug(ctx: &CmdCtx<'_>, raw: &str) -> Result<String> {
    let rest = raw.strip_prefix("@debug").unwrap().trim();
    let sub = rest.split_whitespace().next().unwrap_or("");

    match sub {
        "where" => {
            let s = ctx.sess.lock().await;
            let user = s.name.as_ref().map(|u| u.0.as_str()).unwrap_or("<guest>");
            let msg = match &s.world {
                Some(WorldMode::Live { room_id }) => {
                    format!("[debug] user={user} world=Live room_id={}\r\n", room_id)
                }
                Some(WorldMode::Playtest { bp, room, .. }) => {
                    format!("[debug] user={user} world=Playtest {}:{}\r\n", bp, room)
                }
                None => format!("[debug] user={user} world=None\r\n"),
            };
            Ok(msg)
        }
        _ => Ok("Usage: @debug where\r\n".into()),
    }
}
