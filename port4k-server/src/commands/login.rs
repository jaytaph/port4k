use std::sync::Arc;
use anyhow::anyhow;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use crate::models::account::Account;
use crate::{failure, success};
use crate::db::repo::room::BlueprintAndRoomKey;
use crate::error::DomainError;
use crate::models::zone::{Persistence, ZoneContext, ZoneKind, ZonePolicy};
use crate::renderer::{render_room, render_text, Theme};

const MOTD: &'static str = r#"
====================  PORT4K  ====================
Welcome back, {c:yellow}{v:account.name}{c}!  (last login: {v:last_login})
Server time: {c:white}{v:wall_time}{c}
Location: {c:white}{v:cursor.zone} — {v:cursor.room.title}{c}

Account:  HP {c:white}{v:account.health}/100{c}   XP {c:white}{v:account.xp}{c}   Coins {c:white}{v:account.coins}{c}

News:
 - New vault area unlocked in The Hub.
 - Type 'help' or 'commands' to get started.
 - Use 'who' to see who’s online.

Tips:
 - Most rooms have hidden nouns. Try: {c:blue}'examine terminal'{c}, {c:blue}'open crate'{c}.
 - Use cardinal directions or verbs like {c:blue}'in'{c}/{c:blue}'out'{c} to move.
 - Stuck? Try {c:blue}'look'{c}, {c:blue}'hint'{c}, or {c:blue}'scan'{c}.

Exits from here: {c:blue}{v:room.exits_line}{c}

Enjoy your stay, {v:account.role} {v:account.name}.
=================================================
"#;

pub async fn login(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 3 {
        return Ok(success!("Usage: login <name> <password>\n"));
    }

    // Check if the username is valid
    let (username, pass) = (intent.args[1].as_str(), intent.args[2].as_str());
    if Account::validate_username(username).is_err() {
        return Ok(failure!("Invalid username.\n"));
    }

    let Ok(account) = ctx.registry.services.auth.authenticate(&username, pass).await else {
        return Ok(failure!("Login failed. Check your username and password.\n"));
    };
    ctx.sess.write().account = Some(account.clone());
    ctx.sess.write().state = ConnState::LoggedIn;

    // Either we have a blueprint/room specified in the user account, or we use the default one.
    let zone_key = "hub";
    let bp_key = "hub";
    let room_key = "entry";

    let zone_ctx = match create_zone_context(ctx.clone(), zone_key, bp_key).await {
        Ok(z) => z,
        Err(msg) => return Ok(failure!(msg)),
    };

    // Find the room within the blueprint
    let Ok(room) = ctx.registry.services.blueprint.room_by_key(BlueprintAndRoomKey::new(bp_key, room_key)).await else {
        return Ok(failure!("Error: Starting room not found. Contact admin.\n"));
    };
    ctx.sess.write().zone_ctx = Some(zone_ctx);

    // Generate the cursor
    let c = ctx.registry.services.zone.generate_cursor(ctx.clone(), &account, room.id).await?;
    ctx.sess.write().cursor = Some(c);

    ctx.registry.set_online(&account, true).await;

    let cursor = ctx.cursor().map_err(|_| DomainError::NotFound)?;
    let theme = Theme::blue();
    let width = 80;

    let show_motd = true; // @TODO: Make configurable per-account
    if !show_motd {
        let output = format!(
            "Welcome back, {}! You are in {} — {}.\n",
            account.username, cursor.zone_ctx.zone.title, cursor.room_view.room.title
        );
        return Ok(success!(output));
    }

    // Render the MOTD and the current room
    let output = format!(
        "{}\n{}",
        render_text(ctx.sess.clone(), &theme, width, MOTD),
        render_room(&theme, width, cursor.room_view)
    );
    Ok(success!(output))
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