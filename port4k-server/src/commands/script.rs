use crate::commands::CmdCtx;
use crate::state::session::Editor;
use crate::util::args::{parse_bp_room_key, split_args_quoted};
use anyhow::Result;

pub async fn script(ctx: &CmdCtx<'_>, raw: &str) -> Result<String> {
    let parts = split_args_quoted(raw.trim_start_matches("@script").trim());
    if parts.is_empty() {
        return Ok("Usage:\r\n  @script edit <bp>:<room> <event>\r\n  @script publish <bp>:<room> <event>\r\nNotes:\r\n  End editor with a single line: .end\r\n  Events: on_command | on_enter | on_timer\r\n".into());
    }

    match parts[0].as_str() {
        "edit" if parts.len() >= 3 => {
            let (bp, room) = parse_bp_room_key(&parts[1])
                .ok_or_else(|| anyhow::anyhow!("room must be <bp>:<room>"))?;
            let event = parts[2].to_string();
            let allowed = ["on_command", "on_enter", "on_timer"];
            if !allowed.contains(&event.as_str()) {
                return Ok("Event must be: on_command | on_enter | on_timer\r\n".into());
            }
            {
                let mut s = ctx.sess.lock().await;
                if s.name.is_none() {
                    return Ok("Login required.\r\n".into());
                }
                s.editor = Some(Editor {
                    bp,
                    room,
                    event,
                    buf: String::new(),
                });
            }
            Ok("[editor] Paste your Lua. End with a single line: .end\r\n".into())
        }
        "publish" if parts.len() >= 3 => {
            let (bp, room) = parse_bp_room_key(&parts[1])
                .ok_or_else(|| anyhow::anyhow!("room must be <bp>:<room>"))?;
            let event = parts[2].as_str();
            let ok = ctx.registry.db.bp_script_publish(&bp, &room, event).await?;
            if ok {
                Ok(format!("[script] published {}:{} {}\r\n", bp, room, event))
            } else {
                Ok("[script] no draft found to publish.\r\n".into())
            }
        }
        _ => Ok(
            "Usage:\r\n  @script edit <bp>:<room> <event>\r\n  @script publish <bp>:<room> <event>\r\n"
                .into(),
        ),
    }
}
