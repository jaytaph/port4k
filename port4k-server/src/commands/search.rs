use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::{Intent, NounPhrase};
use std::sync::Arc;

pub async fn search(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if let Some(noun) = intent.direct {
        if let Err(e) = handle_search_object(ctx.clone(), &noun).await {
            // Helpers should already print normal output; we only surface failures.
            ctx.output.system(format!("Error searching '{noun}': {e}")).await;
        }
        return Ok(());
    }

    if let Err(e) = handle_search_room(ctx.clone()).await {
        ctx.output.system(format!("Error searching the room: {e}")).await;
    }

    Ok(())
}

async fn handle_search_object(ctx: Arc<CmdCtx>, noun: &NounPhrase) -> anyhow::Result<()> {
    let rv = ctx.room_view()?;

    if let Some(obj) = rv.object_by_noun(&noun.head) {
        ctx.output
            .line(format!("You search the {} but find nothing of interest.", obj.name))
            .await;
    } else {
        ctx.output
            .line(format!("You see no {} here to search.", noun.head))
            .await;
    }

    Ok(())
}

async fn handle_search_room(ctx: Arc<CmdCtx>) -> anyhow::Result<()> {
    let _rv = ctx.room_view()?;

    ctx.output
        .line("You search the area but find nothing of interest.")
        .await;

    Ok(())
}
