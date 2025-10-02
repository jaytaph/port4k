use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::{ConnState, WorldMode};
use anyhow::Result;
use crate::commands::CommandResult::{Failure, Success};
use crate::db::models::account::Account;

pub async fn login(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() < 3 {
        return Ok(Success("Usage: login <name> <password>\n".into()));
    }

    let (username, pass) = (intent.args[1].as_str(), intent.args[2].as_str());
    if Account::validate_username(username).is_err() {
        return Ok(Failure("Invalid username.\n".into()));
    }

    if !ctx.registry.db.verify_user(&username, pass).await? {
        return Ok(Failure("Invalid credentials.\n".into()))
    }

    let Some(account) = ctx.registry.db.account_by_username(&username).await? else {
        return Ok(Failure("Account not found.\n".into()));
    };

    let (_char_id, loc) = ctx.registry.db.get_or_create_character(account.id, &username).await?;
    {
        let mut s = ctx.sess.write().unwrap();
        s.account = Some(account.clone());
        s.state = ConnState::LoggedIn;
        s.world = Some(WorldMode::Live { room_id: loc });
    }
    ctx.registry.set_online(&account, true).await;
    let view = ctx.registry.db.room_view(loc).await?;

    Ok(Success(format!("Welcome, {}!\n{}", account.username, view)))
}
