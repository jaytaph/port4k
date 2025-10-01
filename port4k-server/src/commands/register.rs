use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use anyhow::Result;
use port4k_core::Username;
use crate::commands::CommandResult::{Failure, Success};

pub async fn register(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.len() < 2 {
        return Ok(Failure("Usage: register <name> <password>\n".into()));
    }

    let (name, pass) = (intent.args[0].as_str(), intent.args[1].as_str());
    let Some(u) = Username::parse(name) else {
        return Ok(Failure("Invalid username.\n".into()));
    };

    if !ctx.registry.db.register_user(&u.0, pass).await? {
        return Ok(Failure("That name is taken.\n".into()))
    }

    Ok(Success(format!(
        "Account `{}` created. You can now `login {} <password>`.\n",
        u, u
    )))
}
