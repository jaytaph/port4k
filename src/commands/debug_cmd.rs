use crate::commands::{CmdCtx, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;

const USAGE: &'static str = "Usage: debug <where|col>\n";

pub async fn debug_cmd(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult {
    if intent.args.len() < 2 {
        ctx.output.system(USAGE).await;
        return Ok(());
    }

    let sub_cmd = intent.args[1].as_str();

    match sub_cmd {
        "col" => {
            ctx.output.system("Color codes:").await;
            let colors = vec![
                "black",
                "red",
                "green",
                "yellow",
                "blue",
                "magenta",
                "cyan",
                "white",
                "bright_black",
                "bright_red",
                "bright_green",
                "bright_yellow",
                "bright_blue",
                "bright_magenta",
                "bright_cyan",
                "bright_white",
            ];
            let mut i = 0;
            let mut line = String::new();
            for fg in &colors {
                for bg in &colors {
                    let s = format!("{{c:{}:{}}}{:02X}{{c}} ", fg, bg, i);
                    line.push_str(&s);
                    i += 1;
                }
                line.push('\n');
            }

            ctx.output.system(line).await;
        }
        "where" => {
            let account = ctx.account()?;
            let username = account.username.clone();

            if !ctx.has_cursor() {
                ctx.output
                    .system("You have no cursor. Use 'go <realm>' to set one.")
                    .await;
            }

            let cursor = ctx.cursor()?;
            ctx.output
                .system(format!(
                    "[debug] user={username} realm={} realm_kind: {:?} room: {}",
                    cursor.realm.title, cursor.realm.kind, cursor.room.blueprint.title
                ))
                .await;
        }
        _ => {
            ctx.output.system("Unknown debug command.").await;
        }
    }

    Ok(())
}
