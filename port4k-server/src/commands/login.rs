use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use crate::models::account::Account;
use crate::{failure, success};
use crate::error::DomainError;
use crate::rendering::{render_room, Theme};

const MOTD: &'static str = r#"
====================  PORT4K  ====================
Welcome back, {username}!  (last login: {last_login})
Server time: {now_local}
Location: {zone_title} — {room_title}

Account:  HP {health}/100   XP {xp}   Coins {coins}
Mail: {unread_messages} unread    Quests: {active_quests}

News:
 - New vault area unlocked in The Hub.
 - Type 'help' or 'commands' to get started.
 - Use 'who' to see who’s online.

Tips:
 - Most rooms have hidden nouns. Try: 'examine terminal', 'open crate'.
 - Use cardinal directions or verbs like 'in'/'out' to move.
 - Stuck? Try 'look', 'hint', or 'scan'.

Exits from here: {exits_line}

Enjoy your stay, {character_name}.
=================================================
"#;

pub async fn login(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    if intent.args.len() < 3 {
        return Ok(success!("Usage: login <name> <password>\n"));
    }

    let (username, pass) = (intent.args[1].as_str(), intent.args[2].as_str());
    if Account::validate_username(username).is_err() {
        return Ok(failure!("Invalid username.\n"));
    }

    let account = ctx.registry.services.auth.authenticate(&username, pass).await?;
    ctx.sess.write().account = Some(account);

    let account = ctx.account()?;
    let (_char_id, _loc) = ctx.registry.db.get_or_create_character(account.id, &username).await?;

    let c = ctx.registry.services.zone.generate_cursor(ctx.clone(), &account, Some("hub"), Some("hub")).await?;
    {
        let mut s = ctx.sess.write();
        s.account = Some(account.clone());  // @TODO: Is this wise? Why clone?
        s.state = ConnState::LoggedIn;
        s.cursor = Some(c);
    }

    // Setup cursor
    ctx.registry.set_online(&account, true).await;

    let cursor = ctx.cursor().map_err(|_| DomainError::NotFound)?;

    let output = format!("{}\n{}", MOTD, render_room(&Theme::blue(), 80, cursor.room_view));
    Ok(success!(output))
}