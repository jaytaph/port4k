use crate::commands::{CmdCtx, CommandResult};
use crate::db::repo::BlueprintAndRoomKey;
use crate::error::DomainError;
use crate::input::parser::Intent;
use crate::models::account::Account;
use crate::models::zone::{Persistence, ZoneContext, ZoneKind, ZonePolicy};
use crate::state::session::ConnState;
use anyhow::anyhow;
use std::sync::Arc;

const MOTD: &str = r#"

** ==============  PORT4K INCOMING MESSAGE =================
**
**   Welcome back, {c:yellow}{v:account.name}{c}!  (last login: {v:last_login:Never logged in before})
**   Server time: {c:white}{v:wall_time}{c}
**
**   Account:
**      HP    : {c:green:bold}{v:account.health:0}/100{c}
**      XP    : {c:green:bold}{v:account.xp:0}{c}
**      Coins : {c:green:bold}{v:account.coins:0}{c}
**
**   News:
**    - New vault area unlocked in The Hub.
**    - Type 'help' or 'commands' to get started.
**    - Use 'who' to see who's online.
**
**   Tips:
**    - Most rooms have hidden nouns. Try: {c:blue}'examine terminal'{c}, {c:blue}'open crate'{c}.
**    - Use cardinal directions or verbs like {c:blue}'in'{c}/{c:blue}'out'{c} to move.
**    - Stuck? Try {c:blue}'look'{c}, {c:blue}'hint'{c}, or {c:blue}'scan'{c}.
**
**   Enjoy your stay, {v:account.role} {v:account.name}.
**
** ====================  END OF MESSAGE ====================

"#;

pub async fn login(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {

    // Step 1: Validate input

    if intent.args.len() < 3 {
        ctx.output.line("Login failed. Check your username and password.").await;
        ctx.output.system("Not enough arguments. Usage: login <user> [pass]").await;
        return Ok(());
    }

    // Step 2: Check if the username is valid

    let (username, pass) = (intent.args[1].as_str(), intent.args[2].as_str());
    match Account::validate_username(username) {
        Ok(_) => {}
        Err(e) => {
            ctx.output.line("Login failed. Check your username and password.").await;
            ctx.output.system(format!("Error: {}", e.to_string())).await;
            return Ok(());
        }
    }

    // Step 3: Authenticate the user

    let account = match ctx.registry.services.auth.authenticate(username, pass).await {
        Ok(acc) => acc,
        Err(e) => {
            ctx.output.line("Login failed. Check your username and password.").await;
            ctx.output.system(format!("Error: {}", e.to_string())).await;
            return Ok(());
        }
    };

    // Step 4: Update session state so we are "logged in".

    ctx.sess.write().account = Some(account.clone());
    ctx.sess.write().state = ConnState::LoggedIn;
    ctx.registry.set_online(&account, true).await;

    // Step 5: Show MOTD if needed

    let show_motd = true; // @TODO: Make configurable per-account
    if show_motd {
        // let vars = RenderVars::new(ctx.sess.clone(), None);
        ctx.output.system(MOTD).await;
    }


    // Step 6: Fetch zone/blueprint/room context and set up cursor

    // Either we have a blueprint/room specified in the user account, or we use the default one.
    let zone_key = "hub";
    let bp_key = "hub";
    let room_key = "entry";

    let zone_ctx = match create_zone_context(ctx.clone(), zone_key, bp_key).await {
        Ok(z) => z,
        Err(e) => {
            ctx.output.line("Login failed due to server error. Contact admin.").await;
            ctx.output.system(format!("Error: {}", e)).await;
            return Ok(());
        }
    };
    ctx.sess.write().zone_ctx = Some(zone_ctx);


    // Step 7: Find the room within the blueprint
    let Ok(room) = ctx
        .registry
        .services
        .blueprint
        .room_by_key(BlueprintAndRoomKey::new(bp_key, room_key))
        .await
    else {
        ctx.output.line("Login failed due to server error. Contact admin.").await;
        ctx.output.system("Room not found").await;
        return Ok(());
    };

    // Step 8: Generate the cursor

    let c = ctx
        .registry
        .services
        .zone
        .generate_cursor(ctx.clone(), &account, room.id)
        .await?;

    // Step 9: Enter the room, run lua hooks if needed
    match ctx.registry.services.room.enter_room(ctx.clone(), &c).await {
        Err(DomainError::RoomNotFound) => {
            ctx.output.line("Login failed due to server error. Contact admin.").await;
            ctx.output.system("Error: Starting room not found.".to_string()).await;
            return Ok(());
        }
        Err(e) => {
            ctx.output.line("Login failed due to server error. Contact admin.").await;
            ctx.output.system(format!("Error: {}", e)).await;
            return Ok(());
        }
        Ok(_) => {}
    }


    // Step 10: Just show the current room after login

    ctx.output.system("You are logged in. Welcome to port4k!".to_string()).await;
    ctx.output.line("You have successfully logged in.").await;
    ctx.output.line(format!("You are in the {}: {}", room.title, room.short.as_deref().unwrap_or("it's not a very descriptive place"))).await;

    Ok(())
}

async fn create_zone_context(ctx: Arc<CmdCtx>, zone_key: &str, bp_key: &str) -> anyhow::Result<ZoneContext> {
    let Ok(Some(zone)) = ctx.registry.services.zone.get_by_key(zone_key).await else {
        return Err(anyhow!("Error: Starting zone not found. Contact admin.\n"));
    };
    let Ok(blueprint) = ctx.registry.services.blueprint.get_by_key(bp_key).await else {
        return Err(anyhow!("Error: Starting blueprint not found. Contact admin.\n"));
    };

    Ok(ZoneContext {
        zone: Arc::new(zone),
        kind: ZoneKind::Live,
        policy: ZonePolicy {
            persistence: Persistence::Persistent,
        },
        blueprint: Arc::new(blueprint),
    })
}
