use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput};
use crate::input::parser::Intent;
use crate::error::AppResult;

pub async fn take(ctx: Arc<CmdCtx>, intent: Intent) -> AppResult<CommandOutput> {
    if intent.args.is_empty() {
        return Ok(Failure("Usage: take coin [N]\n".into()));
    }

    let what = intent.args[0].to_ascii_lowercase();
    if what != "coin" && what != "coins" {
        return Ok(Failure("You can take: coin\n".into()));
    }

    let want: i32 = intent.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let Ok(account) = ctx.account() else {
        return Ok(Failure("Login required.\n".into()));
    };

    let room = {
        let s = ctx.sess.read();
        let room = match &s.cursor {
            Some(c) => c.room.clone(),
            None => return Ok(Failure("You are not in a world.\n".into())),
        };
        room
    };

    let got = ctx.state.registry.db.pickup_coins(&account, room.room.id, want).await?;
    if got == 0 {
        Ok(Failure("There are no coins to pick up.\n".into()))
    } else {
        Ok(Success(format!("You pick up {got} coin(s).\n")))
    }
}
