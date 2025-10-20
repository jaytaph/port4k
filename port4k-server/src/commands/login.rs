use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::db::repo::BlueprintAndRoomKey;
use crate::error::DomainError;
use crate::input::parser::Intent;
use crate::models::account::Account;
use crate::models::zone::{Persistence, ZoneContext, ZoneKind, ZonePolicy};
use crate::renderer::{RenderVars, render_template};
use crate::state::session::ConnState;
use anyhow::anyhow;
use std::sync::Arc;

const MOTD: &str = r#"
==============  PORT4K INCOMING MESSAGE =================
Welcome back, {c:yellow}{v:account.name}{c}!  (last login: {v:last_login})
Server time: {c:white}{v:wall_time}{c}
Location: {c:bright_white}{v:cursor.zone} - {v:cursor.room.title}{c}

Account:  HP {c:green:bold}{v:account.health}/100{c}   XP {c:green:bold}{v:account.xp}{c}   Coins {c:green:bold}{v:account.coins}{c}

News:
 - New vault area unlocked in The Hub.
 - Type 'help' or 'commands' to get started.
 - Use 'who' to see who's online.

Tips:
 - Most rooms have hidden nouns. Try: {c:blue}'examine terminal'{c}, {c:blue}'open crate'{c}.
 - Use cardinal directions or verbs like {c:blue}'in'{c}/{c:blue}'out'{c} to move.
 - Stuck? Try {c:blue}'look'{c}, {c:blue}'hint'{c}, or {c:blue}'scan'{c}.

Enjoy your stay, {v:account.role} {v:account.name}.

====================  END OF MESSAGE ====================

"#;

pub async fn login(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() < 3 {
        out.append("Usage: login <name> <password>\n");
        out.failure();
        return Ok(out);
    }

    // Check if the username is valid
    let (username, pass) = (intent.args[1].as_str(), intent.args[2].as_str());
    if Account::validate_username(username).is_err() {
        out.append("Invalid username.\n");
        out.failure();
        return Ok(out);
    }

    let Ok(account) = ctx.registry.services.auth.authenticate(username, pass).await else {
        out.append("Login failed. Check your username and password.\n");
        out.failure();
        return Ok(out);
    };
    ctx.sess.write().account = Some(account.clone());
    ctx.sess.write().state = ConnState::LoggedIn;

    // Either we have a blueprint/room specified in the user account, or we use the default one.
    let zone_key = "hub";
    let bp_key = "hub";
    let room_key = "entry";

    let zone_ctx = match create_zone_context(ctx.clone(), zone_key, bp_key).await {
        Ok(z) => z,
        Err(msg) => {
            out.append(msg.to_string().as_str());
            out.failure();
            return Ok(out);
        }
    };

    // Find the room within the blueprint
    let Ok(room) = ctx
        .registry
        .services
        .blueprint
        .room_by_key(BlueprintAndRoomKey::new(bp_key, room_key))
        .await
    else {
        out.append("Error: Starting room not found. Contact admin.\n");
        out.failure();
        return Ok(out);
    };
    ctx.sess.write().zone_ctx = Some(zone_ctx);

    // Generate the cursor
    let c = ctx
        .registry
        .services
        .zone
        .generate_cursor(ctx.clone(), &account, room.id)
        .await?;
    ctx.sess.write().cursor = Some(c);

    ctx.registry.set_online(&account, true).await;

    let cursor = ctx.cursor().map_err(|_| DomainError::NotFound)?;
    let width = 80;

    let show_motd = true; // @TODO: Make configurable per-account
    if !show_motd {
        out.append(
            format!(
                "Welcome back, {{c:yellow}}{}{{c}}! You are in {{c:blue:bold}}{}:{}{{c}}.\n",
                account.username, cursor.zone_ctx.zone.title, cursor.room_view.room.title
            )
            .as_str(),
        );
        out.success();
        return Ok(out);
    }

    // Render the MOTD  // and the current room
    let vars = RenderVars::new(ctx.sess.clone(), Some(&cursor.room_view));
    out.append(render_template(MOTD, &vars, width).as_str());
    // out.append(render_room_view(&vars, width).await.as_str());
    out.success();
    Ok(out)
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
