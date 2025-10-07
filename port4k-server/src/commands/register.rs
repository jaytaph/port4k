use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::input::parser::Intent;
use crate::models::account::Account;
use crate::error::AppResult;

pub async fn register(ctx: Arc<CmdCtx>, intent: Intent) -> AppResult<CommandOutput> {
    if intent.args.len() < 2 {
        return Ok(Failure("Usage: register <name> <password>\n".into()));
    }

    let (username, pass) = (intent.args[0].as_str(), intent.args[1].as_str());
    if Account::validate_username(username).is_err() {
        return Ok(Failure("Invalid username.\n".into()));
    }

    if !ctx.state.registry.services.auth.register(&username, pass).await? {
        return Ok(Failure("That name is taken.\n".into()))
    }

    Ok(Success(format!("Account created. You can now `login {} <password>`.\n", username )))
}
