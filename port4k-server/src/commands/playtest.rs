use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::state::session::Cursor;
use crate::{failure, success};
use crate::input::parser::Intent;

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
    if intent.args.len() == 1 {
        // No argument, so exit playtest
        return exit_playtest(ctx.clone()).await;
    }

    if intent.args.len() == 2 {
        // One argument, so enter playtest for the given blueprint
        return enter_playtest(ctx.clone(), intent.args[0].as_str()).await;
    }

    Ok(failure!(USAGE))
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

pub async fn exit_playtest(ctx: Arc<CmdCtx>) -> CommandResult<CommandOutput> {
    match check_playtest(ctx.clone()) {
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
    let account_id = ctx.account_id()?;
    let blueprint = ctx.state.registry.services.blueprint.get_by_key(bp_key).await?;

    let new_c = ctx.state.registry.services.cursor.enter_playtest(account_id, blueprint).await?;
    // let room_view = ctx.state.registry.services.blueprint.get_roomview(&blueprint, &blueprint.start_room).await?;

    match check_playtest(ctx.clone()) {
        Next::NotLoggedIn => Ok(failure!("Login required.\n")),
        Next::ExitToLive(_) => {
            let mut s = ctx.sess.write();
            if let Some(c) = s.cursor.clone() {
                s.prev_cursors.push(c);
            }
            s.cursor = Some(new_c);

            Ok(success!(format!("[playtest] entered recursive blueprint '{bp_key}'.\n")))
        },
        Next::NotInPlaytest => {
            let mut s = ctx.sess.write();
            if let Some(c) = s.cursor.clone() {
                s.prev_cursors.push(c);
            }
            s.cursor = Some(new_c);

            Ok(success!(format!("[playtest] entered blueprint '{bp_key}'.\n")))
        }
    }
}
