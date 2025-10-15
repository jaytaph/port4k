use crate::ConnState;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::renderer::RenderVars;
use crate::renderer::room_view::render_room_view;
use std::sync::Arc;

pub async fn go(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if !ctx.has_cursor() {
        out.append("You are not in a world\n");
        out.failure();
        return Ok(out);
    }

    let Some(dir) = intent.direction else {
        out.append("No direction specified.\n");
        out.append("Usage: go <direction>\n");
        out.failure();
        return Ok(out);
    };

    let c = ctx.cursor()?;
    let account = ctx.account()?;
    let (_from_id, to_id) = ctx.registry.services.navigator.go(&c, account.id, dir).await?;

    let c = ctx
        .registry
        .services
        .zone
        .generate_cursor(ctx.clone(), &account, to_id)
        .await?;
    {
        let mut s = ctx.sess.write();
        s.account = Some(account.clone()); // @TODO: Is this wise? Why clone?
        s.state = ConnState::LoggedIn;
        s.cursor = Some(c);
    }

    // Render the new room
    let c = ctx.cursor()?;

    let vars = RenderVars::new(ctx.sess.clone(), Some(&c.room_view));
    out.append(render_room_view(&vars, 80).await.as_str());
    out.success();
    Ok(out)
}
