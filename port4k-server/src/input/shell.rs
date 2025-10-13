use std::sync::Arc;
use serde::Serialize;
use crate::commands::CmdCtx;
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellCmd {
    Dbg(DbgTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbgTarget {
    RoomView,
    Cursor,
}

pub fn parse_shell_cmd(input: &str) -> Option<ShellCmd> {
    let s = input.trim();
    if !s.starts_with('\\') { return None; }

    // strip leading '\', split on whitespace
    let mut it = s[1..].split_whitespace();
    let cmd = it.next().unwrap_or_default();

    match cmd {
        "dbg" => {
            match it.next().unwrap_or("") {
                "room_view" | "roomview" | "rv" => Some(ShellCmd::Dbg(DbgTarget::RoomView)),
                "cursor"   | "cur"              => Some(ShellCmd::Dbg(DbgTarget::Cursor)),
                "" => {
                    // default target (pick one or show help)
                    Some(ShellCmd::Dbg(DbgTarget::RoomView))
                }
                _ => {
                    // unknown target → still treat as dbg, but you can error in handler
                    Some(ShellCmd::Dbg(DbgTarget::RoomView))
                }
            }
        }
        _ => None, // unrecognized shell command → let game parser try
    }
}


pub async fn handle_shell_cmd(
    cmd: ShellCmd,
    ctx: Arc<CmdCtx>,
) -> AppResult<String> {
    let out = match cmd {
        ShellCmd::Dbg(target) => {
            match target {
                DbgTarget::RoomView => {
                    let room_view = ctx.room_view()?;
                    dump_json_or_debug("room_view", &room_view).await
                }
                DbgTarget::Cursor => {
                    let c = ctx.cursor()?;
                    dump_json_or_debug("cursor", &c).await
                }
            }
        }
    };

    Ok(out)
}

async fn dump_json_or_debug<T: Serialize + core::fmt::Debug>(label: &str, value: &T) -> String {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| format!("{:#?}", value));

    let mut out = String::new();
    out.push_str(format!("--- {label} ---").as_str());
    out.push_str(rendered.as_str());
    out.push_str("--- end ---".into());

    out
}

