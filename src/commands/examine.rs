use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::{Intent, NounPhrase};
use std::sync::Arc;

pub async fn examine(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if let Some(noun) = intent.direct {
        // examine object
        match handle_examine_object(ctx.clone(), &noun).await {
            Ok(_) => {}
            Err(e) => ctx.output.system(format!("Error examining object: {}", e)).await,
        }
    } else {
        ctx.output.system("You must specify what you want to examine.").await;
    }

    Ok(())
}

async fn handle_examine_object(ctx: Arc<CmdCtx>, noun: &NounPhrase) -> anyhow::Result<()> {
    let rv = ctx.room_view()?;
    if let Some(obj) = rv.object_by_noun(&noun.head) {
        match obj.examine.clone() {
            None => {
                ctx.output
                    .line(format!("You examine {}, but you find nothing special.", noun.head).as_str())
                    .await;
            }
            Some(message) => {
                ctx.output.line(message).await;
            }
        }
        return Ok(());
    }

    ctx.output
        .line(format!("You see no {} here to examine.", noun.head))
        .await;

    Ok(())
}
