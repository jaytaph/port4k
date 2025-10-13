pub mod entry;
pub mod exit;
pub mod import;
pub mod new;
pub mod room;
pub mod submit;
mod utils;

use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;

pub async fn blueprint(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.is_empty() {
        out.append(USAGE);
        out.failure();
        return Ok(out);
    }

    let head = intent.args[0].as_str();
    match head {
        "debug_cmd" => new::run(ctx, intent).await,
        "entry" => entry::run(ctx, intent).await,
        "exit" => exit::run(ctx, intent).await,
        "import" => import::run(ctx, intent).await,
        "new" => new::run(ctx, intent).await,
        "playtest" => new::run(ctx, intent).await,
        "room" => room::run(ctx, intent).await,
        "script" => submit::run(ctx, intent).await,
        "submit" => submit::run(ctx, intent).await,
        _ => {
            out.append(USAGE);
            out.failure();
            Ok(out)
        },
    }
}

pub(super) const USAGE: &str = concat!(
    "\x1b[1;36mUsage:\x1b[0m\n",
    "  \x1b[32m@bp\x1b[0m \x1b[1;33mnew\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m ",
    "\x1b[2m\"Title\"\x1b[0m\n",
    "  \x1b[32m@bp\x1b[0m \x1b[1;33mroom add\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m:\x1b[36m<room>\x1b[0m ",
    "\x1b[2m\"Title\" \"Body\"\x1b[0m\n",
    "  \x1b[32m@bp\x1b[0m \x1b[1;33mroom lock\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m:\x1b[36m<room>\x1b[0m\n",
    "  \x1b[32m@bp\x1b[0m \x1b[1;33mroom unlock\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m:\x1b[36m<room>\x1b[0m\n",
    "  \x1b[32m@bp\x1b[0m \x1b[1;33mexit add\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m:\x1b[36m<from>\x1b[0m ",
    "\x1b[35m<dir>\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m:\x1b[36m<to>\x1b[0m ",
    "\x1b[2m[\x1b[35mlocked\x1b[0m\x1b[2m]\x1b[0m\n",
    "  \x1b[32m@bp\x1b[0m \x1b[1;33mentry\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m:\x1b[36m<room>\x1b[0m\n",
    "  \x1b[32m@bp\x1b[0m \x1b[1;33msubmit\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m\n",
    "  \x1b[32m@bp\x1b[0m \x1b[1;33mimport\x1b[0m ",
    "\x1b[36m<bp>\x1b[0m \x1b[36m<dir>\x1b[0m\n",
);
