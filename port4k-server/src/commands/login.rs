use std::sync::Arc;
use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use crate::state::session::ConnState;
use crate::models::account::Account;
use crate::{failure, success};
use crate::error::DomainError;
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

    ctx.registry.set_online(&account, true).await;

    let cursor = ctx.cursor().map_err(|_| DomainError::NotFound)?;

    let theme = Theme::blue();
    let width = 80;

    let output = format!(
        "{}\n{}",
        render_text(ctx.sess.clone(), &theme, width, MOTD),
        render_room(&theme, width, cursor.room_view)
    );
    Ok(success!(output))
}