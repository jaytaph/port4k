use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::models::account::Account;
use crate::{failure, success};

pub async fn register(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 3 {
        return Ok(failure!("Usage: register <name> <email> <password>\n"));
    }

    let (username, email, pass) = (intent.args[0].as_str(), intent.args[1].as_str(), intent.args[2].as_str());
    if Account::validate_username(username).is_err() {
        return Ok(failure!("Invalid username.\n"));
    }

    if !ctx.registry.services.auth.register(&username, email, pass).await? {
        return Ok(failure!("That name or email is taken.\n"))
    }

    Ok(success!(format!("Account created. You can now `login {} <password>`.\n", username )))
}
