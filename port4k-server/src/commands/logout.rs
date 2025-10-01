use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use anyhow::Result;

pub async fn logout(ctx: Arc<CmdCtx>, _intent: Intent) -> Result<String> {
    let mut s = ctx.sess.write().unwrap();
    if s.state == ConnState::PreLogin {
        return Ok("You are already logged out.\n".into());
    }

    s.state = ConnState::PreLogin;
    s.name = None;
    s.world = None;

    Ok("You have been logged out.\n".into())
}
