use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::input::parser::Intent;
use crate::error::AppResult;

pub async fn go(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.is_empty() {
        return Ok(Failure("Usage: go <direction>\n".into()));
    }
    let dir = intent.args[0].to_ascii_lowercase();

    let (account_id, world) = {
        let account_id = ctx.account_id().map_err(|_| anyhow::anyhow!("Not logged in"))?;

        (account_id, s.world.clone())
    };

    match world {
        Some(WorldMode::Live { .. }) => match ctx.state.registry.db.move_character(&account_id, &dir).await? {
            Some(new_room) => {
                {
                    let mut s = ctx.sess.write().unwrap();
                    if let Some(WorldMode::Live { room_id }) = &mut s.world {
                        *room_id = new_room;
                    }
                }
                let view = ctx.state.registry.db.room_view(new_room).await?;
                Ok(Success(view))
            }
            None => Ok(Failure("You can't go that way.\n".into())),
        },
        Some(WorldMode::Playtest { bp, room, .. }) => {
            match ctx.state.registry.db.bp_move(&bp, &room, &dir).await? {
                Some(next) => {
                    {
                        let mut s = ctx.sess.write().unwrap();
                        if let Some(WorldMode::Playtest { room, .. }) = &mut s.world {
                            *room = next.clone();
                        }
                    }
                    // fire on_enter (playtest)
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    ctx.state.lua_tx
                        .send(crate::lua::LuaJob::OnEnterPlaytest {
                            db: ctx.state.registry.db.clone(),
                            blueprint_id: bp.clone(),
                            room_id: next.clone(),
                            account_id: account_id,
                            reply: tx,
                        })
                        .await?;
                    let extra = rx.await??.unwrap_or_default();

                    let view = ctx.state.registry.db
                        .bp_room_view(&bp, &next, 80)
                        .await?
                        .unwrap_or_else(|| "[playtest] room missing\n".into());
                    Ok(Success(format!("{view}{extra}")))
                }
                None => Ok(Failure("You can't go that way (playtest).\n".into())),
            }
        }
        None => Ok(Failure("You are nowhere.\n".into())),
    }
}
