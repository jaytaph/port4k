use crate::state::registry::Registry;
use crate::state::session::Session;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn process_editor_line(
    line: &str,
    registry: &Arc<Registry>,
    sess: &Arc<Mutex<Session>>,
) -> Result<String> {
    // process_editor_line()
    if line.trim() == ".end" {
        let (bp, room, event, src, author) = {
            let mut s = sess.lock().await;
            let ed = s
                .editor
                .take()
                .ok_or_else(|| anyhow::anyhow!("no editor"))?;
            let author = s
                .name
                .as_ref()
                .map(|u| u.0.clone())
                .unwrap_or_else(|| "unknown".into());
            (ed.bp, ed.room, ed.event, ed.buf, author)
        };
        registry
            .db
            .bp_script_put_draft(&bp, &room, &event, &src, &author)
            .await?;
        return Ok(format!(
            "[script] saved draft for {}:{} {}\nUse: @script publish {}:{} {}\n",
            bp, room, event, bp, room, event
        ));
    }

    // accumulate
    {
        let mut s = sess.lock().await;
        if let Some(ed) = &mut s.editor {
            ed.buf.push_str(line);
            ed.buf.push('\n');
        }
    }
    Ok(String::new())
}
