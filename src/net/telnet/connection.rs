use crate::commands::CmdCtx;
use crate::error::AppResult;
use crate::input::readline::{EditEvent, LineEditor};
use crate::lua::table::format_lua_value;
use crate::lua::{LuaJob, LuaResult};
use crate::net::{AppCtx, InputMode};
use crate::util::telnet::{TelnetIn, TelnetMachine};
use crate::{Registry, Session, process_command};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::oneshot;

pub async fn handle_connection(
    read_half: OwnedReadHalf,
    ctx: Arc<AppCtx>,
    telnet: &mut TelnetMachine,
    sess: Arc<RwLock<Session>>,
) -> AppResult<()> {
    let mut reader = BufReader::new(read_half);
    let mut editor = LineEditor::new("> ");

    // Set initial prompt
    ctx.output.restore_prompt().await;

    read_loop(&mut reader, telnet, &mut editor, ctx.clone(), sess.clone()).await?;
    cleanup(sess.clone(), ctx.registry.clone()).await;

    Ok(())
}

async fn read_loop(
    reader: &mut BufReader<OwnedReadHalf>,
    telnet: &mut TelnetMachine,
    editor: &mut LineEditor,
    ctx: Arc<AppCtx>,
    sess: Arc<RwLock<Session>>,
) -> AppResult<()> {
    let mut one = [0u8; 1];

    loop {
        let n = reader.read(&mut one).await?;
        if n == 0 {
            break; // disconnect
        }

        let response = telnet.push(one[0]);

        if let Some(bytes) = response.response {
            ctx.output.raw(bytes).await;
        }

        if let Some(evt) = response.event {
            match evt {
                TelnetIn::Data(b) => handle_data_byte(b, reader, telnet, editor, sess.clone(), ctx.clone()).await?,
                TelnetIn::Naws { cols, rows } => handle_naws(cols, rows, sess.clone()).await,
            }
        }
    }
    Ok(())
}

async fn cleanup(sess: Arc<RwLock<Session>>, registry: Arc<Registry>) {
    let Some(account) = sess.read().get_account() else {
        return;
    };

    registry.set_online(&account, false).await;
}

async fn handle_data_byte(
    b: u8,
    _reader: &mut BufReader<OwnedReadHalf>,
    _telnet: &mut TelnetMachine,
    editor: &mut LineEditor,
    sess: Arc<RwLock<Session>>,
    ctx: Arc<AppCtx>,
) -> AppResult<()> {
    {
        // Set input mask based on session mode
        let s = sess.read();

        // Set current prompt
        editor.set_prompt(s.prompt());

        // Set input masking
        match s.input_mode() {
            InputMode::Normal => editor.set_mask(None),
            InputMode::Hidden(c) => editor.set_mask(Some(c)),
        }
    }

    match editor.handle_byte(b) {
        EditEvent::None => {}
        EditEvent::Redraw => {
            // Redraw because of some input
            ctx.output.draw_line(editor.repaint_line()).await;
        }
        EditEvent::Line(line) => {
            let raw = line.trim();

            let in_repl = sess.read().is_in_lua();
            if in_repl {
                handle_repl_input(raw, ctx.clone(), sess.clone()).await?;
                return Ok(());
            }

            // // Move to a fresh line before emitting any output
            ctx.output.line("\n\n").await;

            // Otherwise, dispatch as a normal command
            dispatch_command(raw, ctx.clone(), sess.clone()).await?;

            // // Update prompt
            // update_prompt(sess.clone(), editor).await;
        }
    }
    Ok(())
}

async fn handle_naws(cols: u16, rows: u16, sess: Arc<RwLock<Session>>) {
    let mut s = sess.write();
    s.set_tty(cols as usize, rows as usize);
}

async fn dispatch_command(raw: &str, ctx: Arc<AppCtx>, sess: Arc<RwLock<Session>>) -> AppResult<()> {
    let cmd_ctx = Arc::new(CmdCtx {
        registry: ctx.registry.clone(),
        output: ctx.output.clone(),
        lua_tx: ctx.lua_tx.clone(),
        sess: sess.clone(),
    });

    match process_command(raw, cmd_ctx.clone()).await {
        Ok(_) => {}
        Err(e) => {
            ctx.output
                .system(format!(
                    "{{c:bright_yellow:bright_red}}Error processing command: {}{{c}}",
                    e
                ))
                .await;
        }
    }
    Ok(())
}

async fn handle_repl_input(raw: &str, ctx: Arc<AppCtx>, sess: Arc<RwLock<Session>>) -> AppResult<()> {
    if matches!(raw, ".quit" | ".exit" | ".q") {
        sess.write().in_lua(false);
        ctx.output.system("Exiting Lua REPL.").await;
        return Ok(());
    }

    if raw == ".help" {
        ctx.output
            .system("Lua REPL commands:\n.quit, .exit, .q - Exit REPL\n.help - Show this help")
            .await;
        return Ok(());
    }

    if raw.is_empty() {
        return Ok(());
    }

    let (reply_tx, reply_rx) = oneshot::channel();

    let Some(c) = sess.read().get_cursor() else {
        ctx.output.system("No active cursor; cannot execute Lua code.").await;
        return Ok(());
    };
    let Some(a) = sess.read().get_account() else {
        ctx.output.system("No active account; cannot execute Lua code.").await;
        return Ok(());
    };

    let job = LuaJob::ReplEval {
        output_handle: ctx.output.clone(),
        cursor: Box::new(c),
        account_id: a.id,
        code: raw.to_string(),
        reply: reply_tx,
    };
    _ = ctx.lua_tx.send(job).await;

    // New line for enter
    ctx.output.line("\n").await;

    match reply_rx.await {
        Ok(LuaResult::Success(value)) => {
            let output = format_lua_value(&value);
            ctx.output.system(output).await;
        }
        Ok(LuaResult::Failed(err)) => {
            ctx.output.system(format!("Lua Error: {}", err)).await;
        }
        Err(_) => {
            ctx.output.system("Failed to receive Lua REPL response.").await;
        }
    }

    Ok(())
}
