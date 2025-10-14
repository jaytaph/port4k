use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::{Intent, NounPhrase};

pub async fn examine(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if let Some(noun) = intent.direct {
        // examine object
        match handle_examine_object(ctx, noun, &mut out).await {
            Ok(_) => {},
            Err(e) => {
                out.append(format!("Error examining object: {}", e).as_str());
                out.failure();
            }
        }
    } else {
        // examine around
        match handle_examine_room(ctx, &mut out).await {
            Ok(_) => {},
            Err(e) => {
                out.append(format!("Error examining room: {}", e).as_str());
                out.failure();
            }
        }
    }

    Ok(out)
}

async fn handle_examine_object(ctx: Arc<CmdCtx>, noun: NounPhrase, out: &mut CommandOutput) -> anyhow::Result<()> {
    let rv = ctx.room_view()?;
    if let Some(obj) = rv.object_by_noun(&noun.head) {
        out.append(&obj.description);
        out.success();
    } else {
        out.append(format!("You see no {} here to examine.", noun.head).as_str());
        out.failure();
    }

    Ok(())
}

async fn handle_examine_room(ctx: Arc<CmdCtx>, out: &mut CommandOutput) -> anyhow::Result<()> {
    let rv = ctx.room_view()?;
    out.append(rv.room.body.as_str());
    out.success();

    Ok(())
}