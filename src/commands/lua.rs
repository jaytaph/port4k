use crate::commands::{CmdCtx, CommandResult};
use std::sync::Arc;

pub async fn repl(ctx: Arc<CmdCtx>) -> CommandResult {
    ctx.output
        .system("Entering Lua REPL... Type '.quit' or '.exit' to leave")
        .await;
    ctx.output.set_prompt("lua> ").await;

    // Mark the session as being in REPL mode
    {
        let mut sess = ctx.sess.write();
        sess.in_lua(true);
    }

    // The actual REPL loop happens in your input handler
    // We'll return here and let the main input loop handle it

    Ok(())
}
