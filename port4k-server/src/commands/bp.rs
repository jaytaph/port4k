use crate::commands::CmdCtx;
use crate::util::args::parse_bp_room_key;
use anyhow::Result;
use std::path::Path;
use crate::input::parser::Intent;

#[allow(unused)]
pub async fn bp(ctx: &CmdCtx, intent: Intent) -> Result<String> {
    if intent.args.is_empty() {
        return Ok("Usage:\r\n  @bp new <bp> \"Title\"\r\n  @bp room add <bp>:<room> \"Title\" \"Body\"\r\n  @bp exit add <bp>:<from> <dir> <bp>:<to>\r\n  @bp entry <bp>:<room>\r\n  @bp submit <bp>\r\n".into());
    }

    match intent.args[0].as_str() {
        "new" if intent.args.len() >= 3 => {
            let bp = &intent.args[1];
            let title = &intent.args[2];
            let owner = ctx.sess.read().unwrap().name.as_ref()
                .ok_or_else(|| anyhow::anyhow!("login required"))?.0.clone();
            if ctx.registry.db.bp_new(bp, title, &owner).await? {
                Ok(format!("[bp] created `{}`: {}\r\n", bp, title))
            } else {
                Ok("[bp] already exists.\r\n".into())
            }
        }
        // @bp room add <bp>:<room> "Title" "Body"
        "room" if intent.args.len() >= 2 => {
            match intent.args[1].as_str() {
                "add" if intent.args.len() >= 5 => {
                    let (bp, room) =
                        parse_bp_room_key(&intent.args[2]).ok_or_else(|| anyhow::anyhow!("room key must be <bp>:<room>"))?;
                    let title = &intent.args[3];
                    let body = &intent.args[4];
                    if ctx.registry.db.bp_room_add(&bp, &room, title, body).await? {
                        Ok(format!("[bp] room {}:{} added.\r\n", bp, room))
                    } else {
                        Ok("[bp] room already exists.\r\n".into())
                    }
                }

                // @bp room lock <bp>:<room>
                "lock" if intent.args.len() >= 3 => {
                    let (bp, room) = parse_bp_room_key(&intent.args[2]).ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
                    if ctx.registry.db.bp_room_set_locked(&bp, &room, true).await? {
                        Ok(format!("[bp] room {}:{} set to LOCKED.\r\n", bp, room))
                    } else {
                        Ok("[bp] blueprint/room not found.\r\n".into())
                    }
                }

                // @bp room unlock <bp>:<room>
                "unlock" if intent.args.len() >= 3 => {
                    let (bp, room) = parse_bp_room_key(&intent.args[2]).ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
                    if ctx.registry.db.bp_room_set_locked(&bp, &room, false).await? {
                        Ok(format!("[bp] room {}:{} set to UNLOCKED.\r\n", bp, room))
                    } else {
                        Ok("[bp] blueprint/room not found.\r\n".into())
                    }
                }

                _ => Ok(USAGE.into()),
            }
        }
        // @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]
        "exit" if intent.args.len() >= 5 && intent.args[1] == "add" => {
            let (bp1, from) =
                parse_bp_room_key(&intent.args[2]).ok_or_else(|| anyhow::anyhow!("from must be <bp>:<room>"))?;
            let dir = intent.args[3].to_ascii_lowercase();
            let (bp2, to) = parse_bp_room_key(&intent.args[4]).ok_or_else(|| anyhow::anyhow!("to must be <bp>:<room>"))?;

            if bp1 != bp2 {
                return Ok("[bp] exits must stay within the same blueprint.\r\n".into());
            }

            // Optional trailing "locked" flag → lock the destination room by default
            let want_locked = intent.args.get(5).map(|s| s.eq_ignore_ascii_case("locked")).unwrap_or(false);

            let mut msg = String::new();
            if ctx.registry.db.bp_exit_add(&bp1, &from, &dir, &to).await? {
                msg.push_str(&format!("[bp] exit {}:{} --{}--> {} added.\r\n", bp1, from, dir, to));
            } else {
                msg.push_str("[bp] exit already exists.\r\n");
            }

            if want_locked {
                match ctx.registry.db.bp_room_set_locked(&bp1, &to, true).await {
                    Ok(true) => msg.push_str(&format!("[bp] room {}:{} set to LOCKED.\r\n", bp1, to)),
                    Ok(false) => msg.push_str("[bp] could not lock destination (room not found?).\r\n"),
                    Err(e) => msg.push_str(&format!("[bp] failed to lock destination: {}\r\n", e)),
                }
            }

            Ok(msg)
        }
        "entry" if intent.args.len() >= 2 => {
            let (bp, room) = parse_bp_room_key(&intent.args[1]).ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
            if ctx.registry.db.bp_set_entry(&bp, &room).await? {
                Ok(format!("[bp] entry set: {}:{}\r\n", bp, room))
            } else {
                Ok("[bp] blueprint not found.\r\n".into())
            }
        }
        "submit" if intent.args.len() >= 2 => {
            let client = ctx.registry.db.pool.get().await?;
            let n = client
                .execute("UPDATE blueprints SET status='submitted' WHERE key=$1", &[&intent.args[1]])
                .await?;
            if n == 1 {
                Ok("[bp] submitted for review.\r\n".into())
            } else {
                Ok("[bp] not found.\r\n".into())
            }
        }
        "import" if intent.args.len() >= 3 => {
            let bp = &intent.args[1];
            let subdir = &intent.args[2];

            // if !ctx.sess.lock().await.is_admin() {
            //     return Ok("[bp] permission denied.\r\n".into());
            // }

            let base_path = Path::new(ctx.registry.config.import_dir.as_str());
            match crate::import::import_blueprint_subdir(bp, subdir, &base_path, &ctx.registry.db).await {
                Ok(()) => Ok(format!(
                    "[bp] imported YAML rooms from {}/{} into `{}`.\r\n",
                    base_path.display(),
                    subdir,
                    bp
                )),
                Err(e) => Ok(format!("[bp] import failed: {:#}\r\n", e)),
            }
        }
        _ => Ok(USAGE.into()),
    }
}

const USAGE: &str = r#"Usage:
  @bp new <bp> "Title"
  @bp room add <bp>:<room> "Title" "Body"
  @bp room lock <bp>:<room>
  @bp room unlock <bp>:<room>
  @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]
  @bp entry <bp>:<room>
  @bp submit <bp>
  @bp import <bp> <dir>
"#;
