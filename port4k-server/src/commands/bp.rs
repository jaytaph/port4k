use anyhow::Result;
use crate::commands::CmdCtx;
use crate::util::args::{split_args_quoted, parse_bp_room_key};

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
        "room" if parts.len() >= 5 && parts[1] == "add" => {
            let (bp, room) = parse_bp_room_key(&parts[2]).ok_or_else(|| anyhow::anyhow!("room key must be <bp>:<room>"))?;
            let title = &parts[3];
            let body = &parts[4];
            if ctx.registry.db.bp_room_add(&bp, &room, title, body).await? {
                Ok(format!("[bp] room {}:{} added.\n", bp, room))
            } else { Ok("[bp] room already exists.\n".into()) }
        }
        "exit" if parts.len() >= 5 && parts[1] == "add" => {
            let (bp1, from) = parse_bp_room_key(&parts[2]).ok_or_else(|| anyhow::anyhow!("from must be <bp>:<room>"))?;
            let dir = parts[3].to_ascii_lowercase();
            let (bp2, to) = parse_bp_room_key(&parts[4]).ok_or_else(|| anyhow::anyhow!("to must be <bp>:<room>"))?;
            if bp1 != bp2 { return Ok("[bp] exits must stay within the same blueprint.\n".into()); }
            if ctx.registry.db.bp_exit_add(&bp1, &from, &dir, &to).await? {
                Ok(format!("[bp] exit {}:{} --{}--> {} added.\n", bp1, from, dir, to))
            } else { Ok("[bp] exit already exists.\n".into()) }
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
        _ => Ok("Usage:\n  @bp new <bp> \"Title\"\n  @bp room add <bp>:<room> \"Title\" \"Body\"\n  @bp exit add <bp>:<from> <dir> <bp>:<to>\n  @bp entry <bp>:<room>\n  @bp submit <bp>\n".into()),
    }
}