use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::state::session::Cursor;
use crate::input::parser::Intent;
use crate::models::zone::ZoneContext;

const USAGE: &str = r#"Usage:
  playtest                # exit playtest
  playtest <bp>           # enter playtest for blueprint <bp>
"#;

pub enum Next {
    /// Contains the cursor to return to live mode
    ExitToLive(Cursor),
    /// Not currently in playtest mode
    NotInPlaytest,
    /// Not logged in
    NotLoggedIn,
}


pub async fn playtest(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() == 1 {
        // No argument, so exit playtest
        _ = exit_playtest(ctx.clone(), &mut out).await;
        return Ok(out);
    }

    if intent.args.len() == 2 {
        // One argument, so enter playtest for the given blueprint
        _ = enter_playtest(ctx.clone(), intent.args[0].as_str(), &mut out).await;
        return Ok(out);
    }

    out.append(USAGE);
    out.failure();
    Ok(out)
}

#[allow(unused)]
pub fn check_playtest(ctx: Arc<CmdCtx>) -> Next {
    if !ctx.is_logged_in() {
        return Next::NotLoggedIn;
    }
    let s = ctx.sess.read();
    if s.prev_cursors.is_empty() {
        return Next::NotInPlaytest;
    }

    let c = s.prev_cursors.first().unwrap().clone();
    Next::ExitToLive(c)
}

pub async fn exit_playtest(ctx: Arc<CmdCtx>, out: &mut CommandOutput) -> anyhow::Result<()> {
    match check_playtest(ctx.clone()) {
        Next::NotLoggedIn => {
            out.append("Login required.\n");
            out.failure();
        },
        Next::NotInPlaytest => {
            out.append("You are not in playtest.\n");
            out.failure();
        },
        Next::ExitToLive(c) => {
            let mut s = ctx.sess.write();
            s.prev_cursors.pop();
            s.cursor = Some(c);

            out.append("[playtest] exited to live mode.\n");
            out.success();
        }
    }

    Ok(())
}

pub async fn enter_playtest(ctx: Arc<CmdCtx>, bp_key: &str, out: &mut CommandOutput) -> anyhow::Result<()> {
    let account_id = ctx.account_id()?;
    let blueprint = Arc::new(ctx.registry.services.blueprint.get_by_key(bp_key).await?);

    let zone_ctx = ZoneContext::ephemeral(account_id, blueprint.clone());
    let new_c = Cursor {
        zone_ctx: zone_ctx.clone(),
        room_id: blueprint.entry_room_id,
        room_view: ctx.registry.services.room.build_room_view(
            ctx.registry.zone_router.clone(),
            &zone_ctx,
            account_id,
            blueprint.entry_room_id,
        ).await?,
    };

    match check_playtest(ctx.clone()) {
        Next::NotLoggedIn => {
            out.append("Login required.\n");
            out.failure();
        },
        Next::ExitToLive(_) => {
            let mut s = ctx.sess.write();
            if let Some(c) = s.cursor.clone() {
                s.prev_cursors.push(c);
            }
            s.cursor = Some(new_c);

            out.append(format!("[playtest] entered recursive blueprint '{bp_key}'.\n").as_str());
            out.success();
        },
        Next::NotInPlaytest => {
            let mut s = ctx.sess.write();
            if let Some(c) = s.cursor.clone() {
                s.prev_cursors.push(c);
            }
            s.cursor = Some(new_c);

            out.append(format!("[playtest] entered blueprint '{bp_key}'.\n").as_str());
            out.success();
        }
    }

    Ok(())
}
