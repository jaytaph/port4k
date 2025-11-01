use crate::commands::CmdCtx;
use crate::error::AppResult;
use crate::input::readline::{EditEvent, LineEditor};
use crate::lua::table::format_lua_value;
use crate::lua::{LuaJob, LuaResult};
use crate::net::AppCtx;
use crate::net::output::OutputHandle;
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

    update_prompt(sess.clone(), ctx.output.clone(), &mut editor).await;

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
    match editor.handle_byte(b) {
        EditEvent::None => {}
        EditEvent::Redraw => {
            update_prompt(sess.clone(), ctx.output.clone(), editor).await;
        }
        EditEvent::Line(line) => {
            let raw = line.trim();

            let in_repl = sess.read().is_in_lua();

            if in_repl {
                handle_repl_input(raw, ctx.clone(), sess.clone()).await?;
                // ctx.output.line("\n\n").await;
                update_prompt(sess.clone(), ctx.output.clone(), editor).await;
                return Ok(());
            }

            // // Move to a fresh line before emitting any output
            ctx.output.line("\n\n").await;

            // // Try login flow first; if handled, we just repaint
            // if try_handle_login(raw, reader, telnet, ctx.clone(), sess.clone()).await? == LoginOutcome::Handled {
            //     update_prompt(sess.clone(), ctx.output.clone(), editor).await;
            //     return Ok(());
            // }

            // Otherwise, dispatch as a normal command
            dispatch_command(raw, ctx.clone(), sess.clone()).await?;

            // Update prompt
            update_prompt(sess.clone(), ctx.output.clone(), editor).await;
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

// #[derive(PartialEq, Eq)]
// enum LoginOutcome {
//     Handled,
//     NotHandled,
// }
//
// /// Return `Handled` if login flow consumed the command
// async fn try_handle_login(
//     raw: &str,
//     reader: &mut BufReader<OwnedReadHalf>,
//     telnet: &mut TelnetMachine,
//     ctx: Arc<AppCtx>,
//     sess: Arc<RwLock<Session>>,
// ) -> AppResult<LoginOutcome> {
//     // Only handle `login <username>` with exactly one arg here
//     let Some(rest) = raw.strip_prefix("login ") else {
//         return Ok(LoginOutcome::NotHandled);
//     };
//     let parts: Vec<&str> = rest.split_whitespace().collect();
//     if parts.len() != 1 {
//         return Ok(LoginOutcome::NotHandled);
//     }
//
//     if sess.read().is_logged_in() {
//         ctx.output.system("Already logged in.").await;
//         return Ok(LoginOutcome::Handled);
//     }
//
//     let username = parts[0];
//
//     if Account::validate_username(username).is_err() {
//         ctx.output.system("Invalid username.").await;
//         return Ok(LoginOutcome::Handled);
//     };
//
//     if !ctx.registry.services.account.exists(username).await? {
//         ctx.output
//             .system("No such user. Try `register <name> <password>`.")
//             .await;
//         return Ok(LoginOutcome::Handled);
//     }
//
//     // We need special handling here to avoid echoing the password
//     ctx.output.system("Password: ").await;
//     let pw = read_secret_line(reader, telnet, ctx.clone()).await?;
//
//     if pw.is_empty() {
//         // Keep the old behavior: end the connection when empty password
//         return Ok(LoginOutcome::Handled);
//     }
//
//     let password = pw.trim_matches(['\r', '\n']);
//     let account = ctx.registry.services.auth.authenticate(username, password).await?;
//
//     sess.write().login(account.clone(), realm, None).await?;
//     ctx.registry.set_online(&account, true).await;
//
//     ctx.output
//         .system(format!("Welcome, {}! Type `look` or `help`.", account.username))
//         .await;
//     Ok(LoginOutcome::Handled)
// }

fn generate_prompt(sess: Arc<RwLock<Session>>, prompt: &str) -> String {
    let s = sess.read();
    if s.is_in_lua() {
        return "lua> ".to_string();
    }

    prompt.to_string()
}

async fn update_prompt(sess: Arc<RwLock<Session>>, output: OutputHandle, editor: &mut LineEditor) {
    let prompt_text = generate_prompt(
        sess.clone(),
        "{c:bright_yellow:blue} {v:account.name:Not logged in} [{rv:title:Nowhere}] @ {v:wall_time} {c} # ",
    );
    editor.set_prompt(&prompt_text);

    output.prompt(editor.repaint_line()).await;
}

/// Read a single line (CR or LF) from the Telnet stream WITHOUT echoing.
/// Backspace/Delete are handled locally. NAWS events update the session.
/// Returns the collected bytes as a String (ASCII/UTF-8 expected).
async fn read_secret_line(
    reader: &mut BufReader<OwnedReadHalf>,
    telnet: &mut TelnetMachine,
    ctx: Arc<AppCtx>,
) -> std::io::Result<String> {
    let mut out = Vec::<u8>::new();
    let mut one = [0u8; 1];

    loop {
        let n = reader.read(&mut one).await?;
        if n == 0 {
            break;
        } // disconnected

        let response = telnet.push(one[0]);

        if let Some(bytes) = response.response {
            ctx.output.raw(bytes).await;
        }

        if let Some(evt) = response.event {
            match evt {
                TelnetIn::Data(b) => {
                    match b {
                        b'\r' | b'\n' => {
                            // Move to the next line visually, but DO NOT reveal password
                            ctx.output.line("\n").await;
                            break;
                        }
                        0x7F | 0x08 => {
                            // Backspace/Delete: remove last byte if any; no echo of backspace either
                            out.pop();
                        }
                        _ => {
                            // Only accept printable ASCII; ignore others for secrets
                            if (0x20..0x7F).contains(&b) {
                                out.push(b);
                            }
                        }
                    }
                }
                TelnetIn::Naws { .. } => {
                    // Ignore NAWS reactive during password entry
                }
            }
        }
    }

    Ok(String::from_utf8_lossy(&out).into_owned())
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
