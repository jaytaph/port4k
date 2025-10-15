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
        out.append("You must specify what you want to examine.");
        out.failure();
    }

    Ok(out)
}

async fn handle_examine_object(ctx: Arc<CmdCtx>, noun: NounPhrase, out: &mut CommandOutput) -> anyhow::Result<()> {
    let rv = ctx.room_view()?;
    if let Some(obj) = rv.object_by_noun(&noun.head) {
        match obj.examine.clone() {
            None => {
                out.append(format!("You examine {}, but you find nothing special.", noun.head).as_str());
                out.success();
            },
            Some(message) => {
                out.append(message.as_str());
                out.success();
            }
        }
        return Ok(());

    }

    out.append(format!("You see no {} here to examine.", noun.head).as_str());
    out.failure();

    Ok(())
}