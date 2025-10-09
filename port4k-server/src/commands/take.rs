use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::{failure, success};

pub async fn take(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.is_empty() {
        return Ok(failure!("Usage: take coin [N]\n"));
    }

    let what = intent.args[0].to_ascii_lowercase();
    if what != "coin" && what != "coins" {
        return Ok(failure!("You can take: coin\n"));
    }

    let want: i32 = intent.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let Ok(account) = ctx.account() else {
        return Ok(failure!("Login required.\n"));
    };

    let room = {
        let s = ctx.sess.read();
        let room = match &s.cursor {
            Some(c) => c.room.clone(),
            None => return Ok(failure!("You are not in a world.\n")),
        };
        room
    };

    let got = ctx.registry.db.pickup_coins(&account, room.room.id, want).await?;
    if got == 0 {
        Ok(failure!("There are no coins to pick up.\n"))
    } else {
        Ok(success!(format!("You pick up {got} coin(s).\n")))
    }
}
