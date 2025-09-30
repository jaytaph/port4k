use crate::commands::CmdCtx;
use crate::state::session::{ConnState, WorldMode};
use anyhow::Result;
use port4k_core::Username;

pub async fn register(ctx: &CmdCtx<'_>, args: Vec<&str>) -> Result<String> {
    if args.len() < 2 {
        return Ok("Usage: register <name> <password>\r\n".into());
    }
    let (name, pass) = (args[0], args[1]);
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

pub async fn login(ctx: &CmdCtx<'_>, args: Vec<&str>) -> Result<String> {
    if args.len() < 2 {
        return Ok("Usage: login <name> <password>\r\n".into());
    }
    let (name, pass) = (args[0], args[1]);
    let Some(u) = Username::parse(name) else {
        return Ok("Invalid username.\r\n".into());
    };
    if ctx.registry.db.verify_user(&u.0, pass).await? {
        let (_char_id, loc) = ctx.registry.db.get_or_create_character(&u.0).await?;
        {
            let mut s = ctx.sess.lock().await;
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
