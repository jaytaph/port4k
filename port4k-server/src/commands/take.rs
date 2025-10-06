use std::sync::Arc;
use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use anyhow::Result;
use crate::commands::CommandResult::{Failure, Success};

pub async fn take(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
    if intent.args.is_empty() {
        return Ok(Failure("Usage: take coin [N]\n".into()));
    }

    let what = intent.args[0].to_ascii_lowercase();
    if what != "coin" && what != "coins" {
        return Ok(Failure("You can take: coin\n".into()));
    }

    let want: i32 = intent.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let (account, room_id) = {
        let s = ctx.sess.read().unwrap();
        let account = match &s.account {
            Some(a) => a.clone(),
            None => return Ok(Failure("You must `login` first.\n".into())),
        };
        let room = match &s.cursor {
            Some(c) => c.room,
            None => return Ok(Failure("You are not in a world.\n".into())),
        };

        (account, room)
    };

    let got = ctx.state.registry.db.pickup_coins(&account, room_id, want).await?;
    if got == 0 {
        Ok(Failure("There are no coins to pick up.\n".into()))
    } else {
        Ok(Success(format!("You pick up {got} coin(s).\n")))
    }
}
