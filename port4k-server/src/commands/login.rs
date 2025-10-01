use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::input::parser::Intent;
use crate::state::session::{ConnState, WorldMode};
use anyhow::Result;
use port4k_core::Username;

pub async fn login(ctx: Arc<CmdCtx>, intent: Intent) -> Result<String> {
    if intent.args.len() < 3 {
        return Ok("Usage: login <name> <password>\r\n".into());
    }
    let (name, pass) = (intent.args[1].as_str(), intent.args[2].as_str());
    let Some(u) = Username::parse(name) else {
        return Ok("Invalid username.\r\n".into());
    };
    if ctx.registry.db.verify_user(&u.0, pass).await? {
        let (_char_id, loc) = ctx.registry.db.get_or_create_character(&u.0).await?;
        {
            let mut s = ctx.sess.write().unwrap();
            s.name = Some(u.clone());
            s.state = ConnState::LoggedIn;
            s.world = Some(WorldMode::Live { room_id: loc });
        }
        ctx.registry.set_online(&u, true).await;
        let view = ctx.registry.db.room_view(loc).await?;
        Ok(format!("Welcome, {}!\r\n{}", u, view))
    } else {
        Ok("Invalid credentials.\r\n".into())
    }
}
