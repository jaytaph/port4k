use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;

pub async fn take(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if intent.args.is_empty() {
        ctx.output.system("Usage: take coin [N]").await;
        return Ok(());
    }

    let what = intent.args[0].to_ascii_lowercase();
    if what != "coin" && what != "coins" {
        ctx.output.system("You can only take coins").await;
        return Ok(());
    }

    let want: i32 = intent.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let Ok(account) = ctx.account() else {
        ctx.output.system("Login required.").await;
        return Ok(());
    };

    let Ok(room_view) = ctx.room_view() else {
        ctx.output.system("You are not in a world.").await;
        return Ok(());
    };

    let got = ctx.registry.db.pickup_coins(&account, room_view.room.id, want).await?;
    if got == 0 {
        ctx.output.line("No coins to pick up.").await;
        return Ok(());
    }

    ctx.output.line(format!("You pick up {got} coin(s).")).await;
    Ok(())
}
