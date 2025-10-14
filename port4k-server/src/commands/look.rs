use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;

pub async fn look(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    let rv = ctx.room_view()?;
    if let Some(noun) = intent.direct {
        if let Some(obj) = rv.object_by_noun(&noun.head) {
            // 1. Check Lua script
            // if let Some(lua_src) = obj.scripts.on_examine_lua.as_ref() {
            //     let reply = run_lua_script(ctx.clone(), lua_src, obj).await?;
            //     return Ok(reply);
            // }

            // 2. Fallback to static description
            out.append(&obj.description);
            out.success();
            return Ok(out);

            // out.append(format!("You see nothing special about the {}.", noun));
            // out.success();
            // return Ok(out)
        }
    }

    // No direct noun -> show room description
    out.append(rv.room.body.as_str());
    out.success();
    Ok(out)
}
