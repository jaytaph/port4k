use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::state::session::{ConnState, WorldMode};
use anyhow::Result;
use crate::commands::blueprint::USAGE;
use crate::commands::CommandResult::{Failure, Success};
use crate::db::types::RoomId;
use crate::input::parser::Intent;

pub async fn playtest(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() < 4 {
        return Ok(Failure(USAGE.into()));
    }

    let sub_args = intent.args[1..].to_vec();

    if sub_args.is_empty() {
        // ----- Scope 1: Touch session, compute next action, DROP the guard
        enum Next { ExitToLive(RoomId), NotInPlaytest }
        let next = {
            let mut s = ctx.sess.write().unwrap();        // <— guard starts
            match &mut s.world {
                Some(WorldMode::Playtest { prev_room_id, .. }) => {
                    let room_id = prev_room_id
                        .take()
                        .ok_or_else(|| anyhow::anyhow!("no previous location"))?;
                    s.world = Some(WorldMode::Live { room_id });
                    Next::ExitToLive(room_id)
                }
                _ => Next::NotInPlaytest,
            }
        }; // <— guard dropped here

        // ----- Scope 2: Now we can await safely
        match next {
            Next::ExitToLive(room_id) => {
                let view = ctx.registry.db.room_view(room_id).await?;
                Ok(Success(format!("[playtest] exited.\n{view}")))
            }
            Next::NotInPlaytest => Ok(Failure("[playtest] you are not in playtest.\n".into())),
        }
    } else {
        // We need entry; fetch it BEFORE locking the session.
        let bp = intent.args[1].as_str();
        let entry = ctx
            .registry
            .db
            .bp_entry(bp)
            .await?
            .ok_or_else(|| anyhow::anyhow!("blueprint has no entry room"))?;

        // ----- Scope 1: Update session state; DROP the guard before await
        let prev_room_opt = {
            let mut s = ctx.sess.write().unwrap();        // <— guard starts

            if s.state != ConnState::LoggedIn {
                // early return without any await, guard will drop at scope end
                return Ok(Failure("Login required.\n".into()));
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

            prev
        }; // <— guard dropped here (even if not used later)

        // ----- Scope 2: Now it’s safe to await
        let view = ctx
            .registry
            .db
            .bp_room_view(bp, &entry, 80)
            .await?
            .unwrap_or_else(|| "[playtest] empty room\n".into());

        Ok(Success(format!("[playtest] entered `{}` at `{}`.\n{}", bp, entry, view)))
    }
}
