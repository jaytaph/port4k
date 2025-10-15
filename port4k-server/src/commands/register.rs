use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::models::account::Account;
use std::sync::Arc;

pub async fn register(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() < 3 {
        out.append("Usage: register <name> <email> <password>\n");
        out.failure();
        return Ok(out);
    }

    let (username, email, pass) = (
        intent.args[0].as_str(),
        intent.args[1].as_str(),
        intent.args[2].as_str(),
    );
    if Account::validate_username(username).is_err() {
        out.append("Invalid username or password.\n");
        out.failure();
        return Ok(out);
    }

    if !ctx.registry.services.auth.register(&username, email, pass).await? {
        out.append("That name or email is taken.\n");
        out.failure();
        return Ok(out);
    }
    out.append("Account created successfully.\n");
    out.append(format!("You can now `login {} <password>`.\n", username).as_str());
    out.success();

    Ok(out)
}
