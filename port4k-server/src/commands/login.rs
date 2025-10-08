use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use crate::models::account::Account;
use crate::{failure, success};

pub async fn login(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 3 {
        return Ok(success!("Usage: login <name> <password>\n"));
    }

    let (username, pass) = (intent.args[1].as_str(), intent.args[2].as_str());
    if Account::validate_username(username).is_err() {
        return Ok(failure!("Invalid username.\n"));
    }

    if !ctx.state.registry.services.auth.authenticate(&username, pass).await? {
        return Ok(failure!("Invalid credentials.\n"))
    }


    let account = ctx.account()?;
    let (_char_id, loc) = ctx.state.registry.db.get_or_create_character(account.id, &username).await?;
    {
        let mut s = ctx.sess.write();
        s.account = Some(account.clone());  // @TODO: Is this wise? Why clone?
        s.state = ConnState::LoggedIn;
        s.cursor = None;
    }
    ctx.state.registry.set_online(&account, true).await;
    let view = ctx.state.registry.db.room_view(loc).await?;

    Ok(success!(format!("Welcome, {}!\n{}", account.username, view)))
}
