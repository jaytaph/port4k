use crate::commands::{CmdCtx, CommandOutput, CommandResult};
use crate::input::parser::Intent;
use std::sync::Arc;
use crate::renderer::{render_template, RenderVars};

const USAGE: &'static str = "Usage: debug <where|col>\n";

pub async fn debug_cmd(ctx: Arc<CmdCtx>, intent: Intent) -> CommandResult<CommandOutput> {
    let mut out = CommandOutput::new();

    if intent.args.len() < 2 {
        out.append(USAGE);
        out.failure();
        return Ok(out);
    }

    let sub_cmd = intent.args[1].as_str();

    match sub_cmd {
        "col" => {
            out.append("Color codes:\n");
            let colors = vec![
                "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
                "bright_black", "bright_red", "bright_green", "bright_yellow",
                "bright_blue", "bright_magenta", "bright_cyan", "bright_white",
            ];
            let mut i = 0;
            let mut line = String::new();
            for fg in &colors {
                for bg in &colors {
                    let s = render_template(&format!("{{c:{}:{}}}{:02X}{{c}} ", fg, bg, i), &RenderVars::default(), 80);
                    line.push_str(&s);
                    i += 1;
                }
                line.push('\n');
            }

            out.append(line.as_str());
            out.success();
        }
        "where" => {
            let account = ctx.account()?;
            let username = account.username;

            if !ctx.has_cursor() {
                out.append("You have no cursor. Use 'go <zone>' to set one.\n");
                out.failure();
            }

            let cursor = ctx.cursor()?;
            out.append(
                format!(
                    "[debug] user={username} zone={} zone_kind: {:?} room: {}\n",
                    cursor.zone_ctx.zone.title, cursor.zone_ctx.kind, cursor.room_view.room.title
                )
                .as_str(),
            );
            out.success();
        }
        _ => {
            out.append("Unknown debug command.\n");
            out.append("Available commands: where\n");
            out.failure();
        }
    }

    Ok(out)
}
