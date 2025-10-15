use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::{Intent, NounPhrase};

pub async fn search(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if let Some(noun) = intent.direct {
        // look at object
        match handle_search_object(ctx, noun, &mut out).await {
            Ok(_) => {},
            Err(e) => {
                out.append(format!("Error searching object: {}", e).as_str());
                out.failure();
            }
        }
    } else {
        // look around
        match handle_search_room(ctx, &mut out).await {
            Ok(_) => {},
            Err(e) => {
                out.append(format!("Error searching room: {}", e).as_str());
                out.failure();
            }
        }
    }

    Ok(out)
}

async fn handle_search_object(ctx: Arc<CmdCtx>, noun: NounPhrase, out: &mut CommandOutput) -> anyhow::Result<()> {
    let rv = ctx.room_view()?;
    if let Some(obj) = rv.object_by_noun(&noun.head) {
        out.append(format!("You search the {} but find nothing of interest.", obj.name).as_str());
        out.success();
    } else {
        out.append(format!("You see no {} here to search.", noun.head).as_str());
        out.failure();
    }

    Ok(())
}

async fn handle_search_room(ctx: Arc<CmdCtx>, out: &mut CommandOutput) -> anyhow::Result<()> {
    let _rv = ctx.room_view()?;
    out.append(format!("You search the area but find nothing of interest.").as_str());
    out.success();

    Ok(())
}
