use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::state::session::Cursor;
use crate::models::zone::{Zone, ZoneKind};
use crate::{failure, success};
use crate::input::parser::Intent;
use crate::services::CommandResult;

const USAGE: &str = r#"Usage:
  playtest                # exit playtest
  playtest <bp>           # enter playtest for blueprint <bp>
"#;

enum Next {
    /// Contains the cursor to return to live mode
    ExitToLive(Cursor),
    /// Not currently in playtest mode
    NotInPlaytest,
    /// Not logged in
    NotLoggedIn,
    /// Some internal error
    Error
}


pub async fn playtest(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() == 1 {
        // No argument, so exit playtest
        return exit_playtest(ctx.clone()).await;
    }

    if intent.args.len() == 2 {
        // One argument, so enter playtest for the given blueprint
        return enter_playtest(ctx.clone(), intent.args[0].as_str()).await;
    }

    Ok(failure!(USAGE.into()))
}

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

pub async fn exit_playtest(ctx: Arc<CmdCtx>) -> CommandResult<CommandOutput> {
    match check_playtest(ctx.clone()) {
        Next::Error => Ok(failure!("Internal error.\n")),
        Next::NotLoggedIn => Ok(failure!("Login required.\n")),
        Next::NotInPlaytest => Ok(failure!("[playtest] you are not in playtest.\n")),
        Next::ExitToLive(c) => {
            let mut s = ctx.sess.write();
            s.prev_cursors.pop();
            s.cursor = Some(c);

            Ok(success!("[playtest] exited.\n"))
        }
    }
}

pub async fn enter_playtest(ctx: Arc<CmdCtx>, bp_key: &str) -> CommandResult<CommandOutput> {
    // Load blueprint
    // Get entry of blueprint
    // Create new zone

    let account_id = ctx.account_id()?;
    let blueprint = ctx.state.registry.services.blueprint.get_by_key(bp_key).await?;

    let new_c = Cursor{
        zone: Zone::ephemeral(),
        zone_kind: ZoneKind::Test { owner: account_id },      // @TODO: This will go wrong
        bp: blueprint,
        room: RoomView {},
    };

    match check_playtest(ctx.clone()) {
        Next::Error => Ok(failure!("Internal error.\n".into())),
        Next::NotLoggedIn => Ok(failure!("Login required.\n".into())),
        Next::ExitToLive(_) => {
            let s = ctx.sess.write();
            if let Some(c) = s.cursor.clone() {
                s.prev_cursors.push(c);
            }
            s.cursor = Some(new_c);

            Ok(success!(format!("[playtest] entered recursive blueprint '{bp_key}'.\n")))
        },
        Next::NotInPlaytest => {
            let s = ctx.sess.write();
            if let Some(c) = s.cursor.clone() {
                s.prev_cursors.push(c);
            }
            s.cursor = Some(new_c);

            Ok(success!(format!("[playtest] entered blueprint '{bp_key}'.\n")))
        }
    }
}
