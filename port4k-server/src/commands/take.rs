use anyhow::Result;
use crate::commands::CmdCtx;
use crate::state::session::WorldMode;

pub async fn take(ctx: &CmdCtx<'_>, args: Vec<&str>) -> Result<String> {
    if args.is_empty() {
        return Ok("Usage: take coin [N]\n".into());
    }
    let what = args[0].to_ascii_lowercase();
    if what != "coin" && what != "coins" {
        return Ok("You can take: coin\n".into());
    }

    let want: i32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let (user, loc) = {
        let s = ctx.sess.lock().await;
        let user = match &s.name { Some(u) => u.clone(), None => return Ok("You must `login` first.\n".into()) };
        match &s.world {
            Some(WorldMode::Live { room_id }) => (user, *room_id),
            Some(WorldMode::Playtest { .. }) => {
                return Ok("[playtest] Coins aren’t available in playtest instances.\n".into());
            }
            None => return Ok("You are nowhere.\n".into()),
        }
    };

    let got = ctx.registry.db.pickup_coins(&user.0, loc, want).await?;
    if got == 0 {
        Ok("There are no coins to pick up.\n".into())
    } else {
        Ok(format!("You pick up {got} coin(s).\n"))
    }
}