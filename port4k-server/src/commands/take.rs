use std::sync::Arc;
use crate::commands::CmdCtx;
use crate::input::parser::Intent;
use crate::state::session::WorldMode;
use anyhow::Result;

pub async fn take(ctx: Arc<CmdCtx>, intent: Intent) -> Result<String> {
    if intent.args.is_empty() {
        return Ok("Usage: take coin [N]\r\n".into());
    }
    let what = intent.args[0].to_ascii_lowercase();
    if what != "coin" && what != "coins" {
        return Ok("You can take: coin\r\n".into());
    }

    let want: i32 = intent.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let (user, loc) = {
        let s = ctx.sess.read().unwrap();
        let user = match &s.name {
            Some(u) => u.clone(),
            None => return Ok("You must `login` first.\r\n".into()),
        };
        match &s.world {
            Some(WorldMode::Live { room_id }) => (user, *room_id),
            Some(WorldMode::Playtest { .. }) => {
                return Ok("[playtest] Coins aren’t available in playtest instances.\r\n".into());
            }
            None => return Ok("You are nowhere.\r\n".into()),
        }
    };

    let got = ctx.registry.db.pickup_coins(&user.0, loc, want).await?;
    if got == 0 {
        Ok("There are no coins to pick up.\r\n".into())
    } else {
        Ok(format!("You pick up {got} coin(s).\r\n"))
    }
}
