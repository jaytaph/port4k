// use std::sync::Arc;
// use crate::commands::{CmdCtx, CommandResult};
// use crate::util::args::parse_bp_room_key;
// use anyhow::Result;
// use crate::commands::CommandResult::{Failure, Success};
// use crate::input::parser::Intent;
//
// pub async fn script(ctx: Arc<CmdCtx>, intent: Intent) -> Result<CommandResult> {
//     if intent.args.len() < 3 {
//         return Ok(failure!("Usage:\n  @script edit <bp>:<room> <event>\n  @script publish <bp>:<room> <event>\nNotes:\n  End editor with a single line: .end\n  Events: on_command | on_enter | on_timer\n".into()));
//     }
//
//     let sub_cmd = intent.args[1].as_str();
//     let sub_args = intent.args[2..].to_vec();
//
//     match sub_cmd {
//         "edit" if sub_args.len() >= 3 => {
//             let (bp, room) = parse_bp_room_key(&sub_args[0]).ok_or_else(|| anyhow::anyhow!("room must be <bp>:<room>"))?;
//             let event = sub_args[1].to_string();
//
//             let allowed = ["on_command", "on_enter", "on_timer"];
//             if !allowed.contains(&event.as_str()) {
//                 return Ok(failure!("Event must be: on_command | on_enter | on_timer\n".into()));
//             }
//
//             {
//                 let mut s = ctx.sess.write().unwrap();
//                 if s.account.is_none() {
//                     return Ok(failure!("Login required.\n".into()));
//                 }
//                 s.editor = Some(Editor { bp, room, event, buf: String::new() });
//             }
//
//             Ok(failure!("[editor] Paste your Lua. End with a single line: .end\n".into()))
//         }
//         "publish" if sub_args.len() >= 3 => {
//             let (bp, room) = parse_bp_room_key(&sub_args[0]).ok_or_else(|| anyhow::anyhow!("room must be <bp>:<room>"))?;
//             let event = sub_args[1].as_str();
//
//             let ok = ctx.registry.db.bp_script_publish(&bp, &room, event).await?;
//             if ok {
//                 Ok(success!(format!("[script] published {}:{} {}\n", bp, room, event)))
//             } else {
//                 Ok(failure!("[script] no draft found to publish.\n".into()))
//             }
//         }
//         _ => Ok(failure!("Usage:\n  @script edit <bp>:<room> <event>\n  @script publish <bp>:<room> <event>\n".into())),
//     }
// }
