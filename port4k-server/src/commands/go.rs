use anyhow::Result;
use crate::commands::CmdCtx;
use crate::state::session::WorldMode;

pub async fn go(ctx: &CmdCtx<'_>, args: Vec<&str>) -> Result<String> {
    if args.is_empty() {
        return Ok("Usage: go <direction>\n".into());
    }
    let dir = args[0].to_ascii_lowercase();

    let (username, world) = {
        let s = ctx.sess.lock().await;
        let username = match &s.name { Some(u) => u.0.clone(), None => return Ok("You must `login` first.\n".into()) };
        (username, s.world.clone())
    };

    match world {
        Some(WorldMode::Live { .. }) => {
            match ctx.registry.db.move_character(&username, &dir).await? {
                Some(new_room) => {
                    {
                        let mut s = ctx.sess.lock().await;
                        if let Some(WorldMode::Live { room_id }) = &mut s.world {
                            *room_id = new_room;
                        }
                    }
                    let view = ctx.registry.db.room_view(new_room).await?;
                    Ok(view)
                }
                None => Ok("You can't go that way.\n".into()),
            }
        }
        Some(WorldMode::Playtest { bp, room, .. }) => {
            match ctx.registry.db.bp_move(&bp, &room, &dir).await? {
                Some(next) => {
                    {
                        let mut s = ctx.sess.lock().await;
                        if let Some(WorldMode::Playtest { room, .. }) = &mut s.world {
                            *room = next.clone();
                        }
                    }
                    // fire on_enter (playtest)
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    ctx.lua_tx.send(crate::lua::LuaJob::OnEnterPlaytest {
                        db: ctx.registry.db.clone(),
                        bp: bp.clone(),
                        room: next.clone(),
                        account: username.clone(),
                        reply: tx,
                    }).await?;
                    let extra = rx.await??.unwrap_or_default();

                    let view = ctx.registry
                        .db
                        .bp_room_view(&bp, &next)
                        .await?
                        .unwrap_or_else(|| "[playtest] room missing\n".into());
                    Ok(format!("{view}{extra}"))
                }
                None => Ok("You can't go that way (playtest).\n".into()),
            }
        }
        None => Ok("You are nowhere.\n".into()),
    }
}