use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::models::account::Account;
use crate::net::InputMode;
use crate::state::interactive::{InteractiveState, RegisterState};
use std::sync::Arc;

pub async fn register(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if let Some(username) = intent.args.first() {
        let st = RegisterState {
            username: Some(username.to_string()),
            email: None,
            password: None,
        };
        ctx.set_interactive(InteractiveState::Register(st));
        ctx.output.set_prompt("Please enter your email: ").await;
        return Ok(());
    }

    ctx.set_interactive(InteractiveState::Register(RegisterState::default()));
    ctx.output.set_prompt("Choose a username: ").await;
    Ok(())
}

pub async fn continue_register(ctx: Arc<CmdCtx>, mut st: RegisterState, raw: &str) -> CommandResult {
    let line = raw.trim();

    if st.username.is_none() {
        if line.is_empty() {
            ctx.output.system("Username cannot be empty.").await;
            return Ok(());
        }

        let username = line.to_string();
        if Account::validate_username(&username).is_err() {
            ctx.output.system("Invalid username or password.").await;
            return Ok(());
        }
        if ctx.registry.services.account.exists(&username).await? {
            ctx.output.system("That username is already taken.").await;
            return Ok(());
        }

        st.username = Some(line.to_string());
        ctx.set_interactive(InteractiveState::Register(st));
        ctx.output.set_prompt("Please enter your email: ").await;
        return Ok(());
    }

    if st.email.is_none() {
        if line.is_empty() {
            ctx.output.system("Email cannot be empty.").await;
            return Ok(());
        } else {
            let email = line.to_string();
            if ctx.registry.services.account.exists_email(&email).await? {
                ctx.output.system("That email is already taken.").await;
                return Ok(());
            }

            st.email = Some(line.to_string());
            ctx.set_interactive(InteractiveState::Register(st));
            ctx.output.set_prompt("Please enter your password: ").await;
            ctx.output.input_mode(InputMode::Hidden('*')).await;
            return Ok(());
        }
    }
    if st.password.is_none() {
        if line.is_empty() {
            ctx.output.system("Password cannot be empty.").await;
            return Ok(());
        } else {
            st.password = Some(line.to_string());
        }
    }

    // Proceed to register the account
    let _username = st.username.clone().unwrap();
    let _email = st.email.clone().unwrap();
    let _password = st.password.clone().unwrap();

    ctx.output.system("Creating account....").await;
    ctx.set_interactive(InteractiveState::None);

    Ok(())
}
