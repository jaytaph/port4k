use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use crate::renderer::room_view::render_room_view;
use std::sync::Arc;

pub async fn look(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    let rv = ctx.room_view()?;
    if let Some(noun) = intent.direct {
        return if let Some(obj) = rv.object_by_noun(&noun.head) {
            // 1. Check Lua script
            // if let Some(lua_src) = obj.scripts.on_examine_lua.as_ref() {
            //     let reply = run_lua_script(ctx.clone(), lua_src, obj).await?;
            //     return Ok(reply);
            // }

            // 2. Fallback to static description
            ctx.output.system(&obj.description).await;
            Ok(())

            // out.append(format!("You see nothing special about the {}.", noun));
            // out.success();
            // return Ok(out)
        } else {
            ctx.output.system(format!("You don't see any '{}' here.", noun.head)).await;
            Ok(())
        };
    }

    // No direct noun -> show room description
    // let vars = RenderVars::new(ctx.sess.clone(), Some(&rv));
    ctx.output.line(render_room_view()).await;
    Ok(())
}
