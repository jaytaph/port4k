use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;

pub async fn take(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.is_empty() {
        out.append("Usage: take coin [N]\n");
        out.failure();
        return Ok(out);
    }

    let what = intent.args[0].to_ascii_lowercase();
    if what != "coin" && what != "coins" {
        out.append("You can only take coins\n");
        out.failure();
        return Ok(out);
    }

    let want: i32 = intent.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let Ok(account) = ctx.account() else {
        out.append("Login required.\n");
        out.failure();
        return Ok(out);
    };

    let Ok(room_view) = ctx.room_view() else {
        out.append("You are not in a world\n");
        out.failure();
        return Ok(out);
    };

    let got = ctx.registry.db.pickup_coins(&account, room_view.room.id, want).await?;
    if got == 0 {
        out.append("No coins to pick up\n");
        out.failure();
        return Ok(out);
    }

    out.append(format!("You pick up {got} coin(s).\n").as_str());
    out.success();
    Ok(out)
}
