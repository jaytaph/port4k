use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::models::account::Account;
use std::sync::Arc;

pub async fn register(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if ctx.is_logged_in() {
        ctx.output
            .system("You are already logged in. Logout before registering a new account.")
            .await;
        return Ok(());
    }

    if intent.args.len() < 3 {
        ctx.output.system("Usage: register <name> <email> <password>").await;
        return Ok(());
    }

    let (username, _email, _pass) = (
        intent.args[0].as_str(),
        intent.args[1].as_str(),
        intent.args[2].as_str(),
    );
    if Account::validate_username(username).is_err() {
        ctx.output.system("Invalid username or password.").await;
        return Ok(());
    }

    // if !ctx.registry.services.auth.register(username, email, pass).await? {
    //     ctx.output.system("That name or email is taken.").await;
    //     return Ok(());
    // }
    // ctx.output.system("Account created successfully.").await;
    // ctx.output
    //     .system(format!("You can now `login {} <password>`.", username))
    //     .await;

    Ok(())
}
