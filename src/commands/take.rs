use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use rand::Rng;
use std::sync::Arc;

pub async fn take(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if intent.args.is_empty() {
        ctx.output.system("Usage: take <item> [N]").await;
        return Ok(());
    }

    let what = intent.direct.unwrap().head;

    // // Special case: coins (the only takeable thing)
    // if what == "coin" || what == "coins" {
    //     return take_coins(ctx, &intent.args).await;
    // }

    // Check if it's an object in the room (can't be taken)
    let Ok(room_view) = ctx.room_view() else {
        ctx.output.system("You are not in a world.").await;
        return Ok(());
    };

    // Check if this thing exists as an object in the room
    let is_known_object = room_view
        .objects
        .iter()
        .any(|obj| obj.name.to_ascii_lowercase().contains(&what));

    if is_known_object {
        // It exists but can't be taken
        let messages = [
            "You can't take that.",
            "That's not something you can pick up.",
            "You can't carry that around.",
            "That's firmly in place.",
            "It's too heavy to lift.",
            "You try, but it won't budge.",
            "That's not going anywhere.",
            "Better leave that where it is.",
            "You can't just take everything you see.",
            "That's attached to something.",
            "You tug at it, but it's stuck fast.",
            "It's part of the scenery.",
            "That would be impractical to carry.",
            "You'd need a forklift for that.",
            "Your hands aren't big enough for that.",
            "It's bolted down.",
            "That's not yours to take.",
            "You decide to leave it be.",
            "It looks permanently mounted.",
            "That's way too unwieldy.",
        ];
        let msg = messages[rand::rng().random_range(0..messages.len())];
        ctx.output.line(msg).await;
    } else {
        // Unknown thing
        let messages = [
            format!("I don't know what '{}' is.", what),
            format!("You don't see any '{}' here.", what),
            format!("What's a '{}'?", what),
            "You don't see that here.".to_string(),
            format!("There's no '{}' around.", what),
            format!("A '{}'? Not here.", what),
            format!("You look around but don't see any '{}'.", what),
            format!("'{}' isn't something you recognize.", what),
            "You don't see anything like that.".to_string(),
            format!("Never heard of a '{}'.", what),
            format!("There's nothing called '{}' here.", what),
            "That doesn't seem to exist.".to_string(),
            format!("You search but find no '{}'.", what),
            format!("A '{}' would be nice, but there isn't one.", what),
            "Nothing by that name here.".to_string(),
            format!("You squint, but still no '{}'.", what),
            format!("Maybe '{}' exists somewhere, but not here.", what),
            "You draw a blank.".to_string(),
            format!("'{}' is not in your vicinity.", what),
            format!("You're pretty sure '{}' isn't a thing.", what),
        ];
        let msg = messages[rand::rng().random_range(0..messages.len())].to_string();
        ctx.output.line(msg).await;
    }

    Ok(())
}
