use std::path::Path;
use crate::commands::CmdCtx;
use crate::util::args::{parse_bp_room_key, split_args_quoted};
use anyhow::Result;

pub async fn bp(ctx: &CmdCtx<'_>, raw: &str) -> Result<String> {
    let rest = raw.strip_prefix("@bp").unwrap().trim();
    let parts = split_args_quoted(rest);
    if parts.is_empty() {
        return Ok("Usage:\n  @bp new <bp> \"Title\"\n  @bp room add <bp>:<room> \"Title\" \"Body\"\n  @bp exit add <bp>:<from> <dir> <bp>:<to>\n  @bp entry <bp>:<room>\n  @bp submit <bp>\n".into());
    }

    match parts[0].as_str() {
        "new" if parts.len() >= 3 => {
            let bp = &parts[1];
            let title = &parts[2];
            let owner = ctx.sess.lock().await.name.as_ref().ok_or_else(|| anyhow::anyhow!("login required"))?.0.clone();
            if ctx.registry.db.bp_new(bp, title, &owner).await? {
                Ok(format!("[bp] created `{}`: {}\n", bp, title))
            } else { Ok("[bp] already exists.\n".into()) }
        }
        // @bp room add <bp>:<room> "Title" "Body"
        "room" if parts.len() >= 2 => {
            match parts[1].as_str() {
                "add" if parts.len() >= 5 => {
                    let (bp, room) = parse_bp_room_key(&parts[2])
                        .ok_or_else(|| anyhow::anyhow!("room key must be <bp>:<room>"))?;
                    let title = &parts[3];
                    let body = &parts[4];
                    if ctx.registry.db.bp_room_add(&bp, &room, title, body).await? {
                        Ok(format!("[bp] room {}:{} added.\n", bp, room))
                    } else {
                        Ok("[bp] room already exists.\n".into())
                    }
                }

                // @bp room lock <bp>:<room>
                "lock" if parts.len() >= 3 => {
                    let (bp, room) = parse_bp_room_key(&parts[2])
                        .ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
                    if ctx.registry.db.bp_room_set_locked(&bp, &room, true).await? {
                        Ok(format!("[bp] room {}:{} set to LOCKED.\n", bp, room))
                    } else {
                        Ok("[bp] blueprint/room not found.\n".into())
                    }
                }

                // @bp room unlock <bp>:<room>
                "unlock" if parts.len() >= 3 => {
                    let (bp, room) = parse_bp_room_key(&parts[2])
                        .ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
                    if ctx.registry.db.bp_room_set_locked(&bp, &room, false).await? {
                        Ok(format!("[bp] room {}:{} set to UNLOCKED.\n", bp, room))
                    } else {
                        Ok("[bp] blueprint/room not found.\n".into())
                    }
                }

                _ => Ok(USAGE.into()),
            }
        }
        // @bp exit add <bp>:<from> <dir> <bp>:<to> [locked]
        "exit" if parts.len() >= 5 && parts[1] == "add" => {
            let (bp1, from) = parse_bp_room_key(&parts[2])
                .ok_or_else(|| anyhow::anyhow!("from must be <bp>:<room>"))?;
            let dir = parts[3].to_ascii_lowercase();
            let (bp2, to) = parse_bp_room_key(&parts[4])
                .ok_or_else(|| anyhow::anyhow!("to must be <bp>:<room>"))?;

            if bp1 != bp2 {
                return Ok("[bp] exits must stay within the same blueprint.\n".into());
            }

            // Optional trailing "locked" flag → lock the destination room by default
            let want_locked = parts.get(5).map(|s| s.eq_ignore_ascii_case("locked")).unwrap_or(false);

            let mut msg = String::new();
            if ctx.registry.db.bp_exit_add(&bp1, &from, &dir, &to).await? {
                msg.push_str(&format!("[bp] exit {}:{} --{}--> {} added.\n", bp1, from, dir, to));
            } else {
                msg.push_str("[bp] exit already exists.\n");
            }

            if want_locked {
                match ctx.registry.db.bp_room_set_locked(&bp1, &to, true).await {
                    Ok(true)  => msg.push_str(&format!("[bp] room {}:{} set to LOCKED.\n", bp1, to)),
                    Ok(false) => msg.push_str("[bp] could not lock destination (room not found?).\n"),
                    Err(e)    => msg.push_str(&format!("[bp] failed to lock destination: {}\n", e)),
                }
            }

            Ok(msg)
        }
        "entry" if parts.len() >= 2 => {
            let (bp, room) = parse_bp_room_key(&parts[1]).ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
            if ctx.registry.db.bp_set_entry(&bp, &room).await? {
                Ok(format!("[bp] entry set: {}:{}\n", bp, room))
            } else { Ok("[bp] blueprint not found.\n".into()) }
        }
        "submit" if parts.len() >= 2 => {
            let client = ctx.registry.db.pool.get().await?;
            let n = client.execute("UPDATE blueprints SET status='submitted' WHERE key=$1", &[&parts[1]]).await?;
            if n == 1 { Ok("[bp] submitted for review.\n".into()) } else { Ok("[bp] not found.\n".into()) }
        }
        "import" if parts.len() >= 3 => {
            let bp = &parts[1];
            let subdir = &parts[2];

            // if !ctx.sess.lock().await.is_admin() {
            //     return Ok("[bp] permission denied.\n".into());
            // }

            let base_path = Path::new(ctx.registry.config.import_dir.as_str());
            match crate::import::import_blueprint_subdir(bp, subdir, &base_path, &ctx.registry.db).await {
                Ok(()) => Ok(format!("[bp] imported YAML rooms from {}/{} into `{}`.\n", base_path.display(), subdir, bp)),
                Err(e) => Ok(format!("[bp] import failed: {:#}\n", e)),
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

