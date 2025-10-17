use crate::banner::{BANNER, ENTRY};
use crate::commands::{CmdCtx, CommandOutput};
use crate::error::AppResult;
use crate::input::readline::{EditEvent, LineEditor};
use crate::models::account::Account;
use crate::net::AppCtx;
use crate::net::telnet::crlf_wrapper::CrlfWriter;
use crate::net::telnet::slow_writer::{Pace, SlowWriter};
use crate::renderer::{RenderVars, render_template};
use crate::util::telnet::{TelnetIn, TelnetMachine};
use crate::{ConnState, Registry, Session, process_command};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedReadHalf;

pub async fn handle_connection(stream: TcpStream, ctx: Arc<AppCtx>, sess: Arc<RwLock<Session>>) -> AppResult<()> {
    // Split stream into read/write halves and wrap write half to ensure CRLF outputs.
    let (r, w) = stream.into_split();
    let crlf_w = CrlfWriter::new(w);
    let mut writer = SlowWriter::new(
        crlf_w,
        Pace::PerWord {
            delay: Duration::from_millis(1),
        },
    );

    let mut reader = BufReader::new(r);
    let mut editor = LineEditor::new("> ");
    let mut telnet = TelnetMachine::new();

    start_session(&mut writer, &mut telnet, &mut editor, sess.clone()).await?;
    read_loop(
        &mut reader,
        &mut writer,
        &mut telnet,
        &mut editor,
        ctx.clone(),
        sess.clone(),
    )
    .await?;
    cleanup(sess.clone(), ctx.registry.clone()).await;

    Ok(())
}

//noinspection RsExternalLinter
async fn start_session<W: AsyncWrite + Unpin>(
    w: &mut SlowWriter<W>,
    telnet: &mut TelnetMachine,
    editor: &mut LineEditor,
    sess: Arc<RwLock<Session>>,
) -> AppResult<()> {
    // Telnet option negotiation: character-at-a-time + SGA + (server) echo + NAWS
    telnet.start_negotiation(w).await?;

    // Welcome text
    if !BANNER.is_empty() {
        for line in BANNER.lines() {
            w.write_all(line.as_bytes()).await?;
            w.write_all(b"\n").await?;
        }
    }
    if !ENTRY.is_empty() {
        for line in ENTRY.lines() {
            w.write_all(line.as_bytes()).await?;
            w.write_all(b"\n").await?;
        }
    }

    repaint_prompt(sess.clone(), w, editor, true).await?;
    Ok(())
}

async fn read_loop<W: AsyncWrite + Unpin>(
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut SlowWriter<W>,
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

        if let Some(evt) = telnet.push(one[0], w).await? {
            match evt {
                TelnetIn::Data(b) => handle_data_byte(b, reader, w, telnet, editor, sess.clone(), ctx.clone()).await?,
                TelnetIn::Naws { cols, rows } => handle_naws(cols, rows, sess.clone()).await,
            }
        }
    }
    Ok(())
}

fn generate_prompt(sess: Arc<RwLock<Session>>, prompt: &str) -> String {
    // No roomview vars in prompt generation
    let vars = RenderVars::new(sess.clone(), None);
    render_template(prompt, &vars, 80)
}

async fn cleanup(sess: Arc<RwLock<Session>>, registry: Arc<Registry>) {
    let account_opt = {
        let sess = sess.read();
        sess.account.clone()
    };

    if let Some(a) = account_opt {
        registry.set_online(&a, false).await;
    }
}

async fn handle_data_byte<W: AsyncWrite + Unpin>(
    b: u8,
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut SlowWriter<W>,
    telnet: &mut TelnetMachine,
    editor: &mut LineEditor,
    sess: Arc<RwLock<Session>>,
    ctx: Arc<AppCtx>,
) -> AppResult<()> {
    match editor.handle_byte(b) {
        EditEvent::None => {}
        EditEvent::Redraw => {
            repaint_prompt(sess.clone(), w, editor, false).await?;
        }
        EditEvent::Line(line) => {
            // Move to a fresh line before emitting any output
            w.write_all(b"\r\n").await?;
            let raw = line.trim();
            tracing::debug!(%raw, "received line");

            // Try login flow first; if handled, we just repaint
            if try_handle_login(raw, reader, w, telnet, ctx.clone(), sess.clone()).await? == LoginOutcome::Handled {
                repaint_prompt(sess.clone(), w, editor, true).await?;
                return Ok(());
            }

            // Otherwise, dispatch as a normal command
            dispatch_command(raw, w, ctx.clone(), sess.clone()).await?;

            // Always repaint prompt after server output
            repaint_prompt(sess.clone(), w, editor, true).await?;
        }
    }
    Ok(())
}

async fn handle_naws(cols: u16, rows: u16, sess: Arc<RwLock<Session>>) {
    let mut s = sess.write();
    s.tty_cols = Some(cols as usize);
    s.tty_rows = Some(rows as usize);
}

async fn dispatch_command<W: AsyncWrite + Unpin>(
    raw: &str,
    w: &mut W,
    ctx: Arc<AppCtx>,
    sess: Arc<RwLock<Session>>,
) -> AppResult<()> {
    let cmd_ctx = Arc::new(CmdCtx {
        registry: ctx.registry.clone(),
        lua_tx: ctx.lua_tx.clone(),
        sess: sess.clone(),
    });

    match process_command(raw, cmd_ctx.clone()).await {
        Ok(res) => {
            if res.succeeded() {
                output_success(w, res).await;
            } else {
                output_error(w, res).await;
            };
            // write_with_newline(w, out.as_bytes()).await?;
        }
        Err(e) => {
            write_with_newline(w, format!("error: {e}").as_bytes()).await?;
        }
    }
    Ok(())
}

async fn output_error<W: AsyncWrite + Unpin>(w: &mut W, out: CommandOutput) {
    let _ = write_with_newline(w, b"\n").await;
    let _ = write_with_newline(w, b"An error occurred while processing your command.").await;
    let _ = write_with_newline(w, b"\n").await;
    for msg in out.messages() {
        let _ = write_with_newline(w, msg.as_bytes()).await;
        let _ = write_with_newline(w, b"\n").await;
    }
    let _ = write_with_newline(w, b"\n").await;
}

async fn output_success<W: AsyncWrite + Unpin>(w: &mut W, out: CommandOutput) {
    let _ = write_with_newline(w, b"\n").await;
    for msg in out.messages() {
        let _ = write_with_newline(w, msg.as_bytes()).await;
        let _ = write_with_newline(w, b"\n").await;
    }
    let _ = write_with_newline(w, b"\n").await;
}

#[derive(PartialEq, Eq)]
enum LoginOutcome {
    Handled,
    NotHandled,
}

/// Return `Handled` if login flow consumed the command
async fn try_handle_login<W: AsyncWrite + Unpin>(
    raw: &str,
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut SlowWriter<W>,
    telnet: &mut TelnetMachine,
    ctx: Arc<AppCtx>,
    sess: Arc<RwLock<Session>>,
) -> AppResult<LoginOutcome> {
    // Only handle `login <username>` with exactly one arg here
    let Some(rest) = raw.strip_prefix("login ") else {
        return Ok(LoginOutcome::NotHandled);
    };
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.len() != 1 {
        return Ok(LoginOutcome::NotHandled);
    }

    if sess.read().state == ConnState::LoggedIn {
        write_with_newline(w, b"Already logged in.").await?;
        return Ok(LoginOutcome::Handled);
    }

    let username = parts[0];

    if Account::validate_username(username).is_err() {
        write_with_newline(w, b"Invalid username.").await?;
        return Ok(LoginOutcome::Handled);
    };

    if !ctx.registry.services.account.exists(username).await? {
        write_with_newline(w, b"No such user. Try `register <name> <password>`.").await?;
        return Ok(LoginOutcome::Handled);
    }
    w.write_all(b"Password: ").await?;
    w.flush().await?;

    let pw = read_secret_line(reader, w, telnet).await?;

    if pw.is_empty() {
        // Keep the old behavior: end the connection when empty password
        return Ok(LoginOutcome::Handled);
    }

    let password = pw.trim_matches(['\r', '\n']);
    let account = ctx.registry.services.auth.authenticate(username, password).await?;

    // // All is ok
    // let Some(account) = state.registry.repos.account.get_by_username(&username).await? else {
    //     write_with_newline(w, "Account retrieval error.").await?;
    //     return Ok(LoginOutcome::Handled);
    // };

    {
        let mut s = sess.write();
        s.account = Some(account.clone());
        s.state = ConnState::LoggedIn;
    }
    ctx.registry.set_online(&account, true).await;

    write_with_newline(
        w,
        format!("Welcome, {}! Type `look` or `help`.", account.username).as_bytes(),
    )
    .await?;
    Ok(LoginOutcome::Handled)
}

async fn repaint_prompt<W: AsyncWrite + Unpin>(
    sess: Arc<RwLock<Session>>,
    w: &mut SlowWriter<W>,
    editor: &mut LineEditor,
    generate_new_prompt: bool,
) -> std::io::Result<()> {
    if generate_new_prompt {
        let prompt = generate_prompt(
            sess.clone(),
            "{c:yellow:red:bold} {v:account.name:Not logged in} [{rv:title:Nowhere}] @ {v:wall_time}{c} # ",
        );
        editor.set_prompt(&prompt);
    }

    w.set_pacing(false);
    w.write_all(editor.repaint_line().as_bytes()).await?;
    w.set_pacing(true);
    w.flush().await
}

/// Write text, ensuring it ends in CRLF (good Telnet hygiene)
async fn write_with_newline<W: AsyncWrite + Unpin>(w: &mut W, bytes: &[u8]) -> std::io::Result<()> {
    w.write_all(bytes).await?;

    if !bytes.ends_with(b"\n") {
        w.write_all(b"\n").await?;
    }
    Ok(())
}

/// Read a single line (CR or LF) from the Telnet stream WITHOUT echoing.
/// Backspace/Delete are handled locally. NAWS events update the session.
/// Returns the collected bytes as a String (ASCII/UTF-8 expected).
async fn read_secret_line<W: AsyncWrite + Unpin>(
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut W,
    telnet: &mut TelnetMachine,
) -> std::io::Result<String> {
    let mut out = Vec::<u8>::new();
    let mut one = [0u8; 1];

    loop {
        let n = reader.read(&mut one).await?;
        if n == 0 {
            break;
        } // disconnect

        if let Some(evt) = telnet.push(one[0], w).await? {
            match evt {
                TelnetIn::Data(b) => {
                    match b {
                        b'\r' | b'\n' => {
                            // Move to the next line visually, but DO NOT reveal password
                            w.write_all(b"\n").await?;
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
                TelnetIn::Naws { cols, rows } => {
                    dbg!(&cols, &rows);
                    // // Keep NAWS reactive even during password entry
                    // let mut s = ctx.sess.lock().await;
                    // s.tty_cols = Some(cols as usize);
                    // s.tty_rows = Some(rows as usize);
                }
            }
        }
    }

    Ok(String::from_utf8_lossy(&out).into_owned())
}
