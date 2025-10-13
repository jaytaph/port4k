use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;

pub async fn look(_ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();
    out.append("you look around. Nothing to see here... yet.");

    for o in intent.objects {
        out.append(&format!("\nYou look at {}. It's fascinating!", o.head));
    }

    out.success();
    Ok(out)
}
