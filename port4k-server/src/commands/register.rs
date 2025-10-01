use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::input::parser::Intent;
use anyhow::Result;
use port4k_core::Username;

pub async fn register(ctx: Arc<CmdCtx>, intent: Intent) -> Result<String> {
    if intent.args.len() < 2 {
        return Ok("Usage: register <name> <password>\r\n".into());
    }
    let (name, pass) = (intent.args[0].as_str(), intent.args[1].as_str());
    let Some(u) = Username::parse(name) else {
        return Ok("Invalid username.\r\n".into());
    };
    if ctx.registry.db.register_user(&u.0, pass).await? {
        Ok(format!(
            "Account `{}` created. You can now `login {} <password>`.\r\n",
            u, u
        ))
    } else {
        Ok("That name is taken.\r\n".into())
    }
}
