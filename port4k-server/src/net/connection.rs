use crate::lua::LuaJob;
use crate::readline::{EditEvent, LineEditor};
use crate::util::telnet::{TelnetIn, TelnetMachine};
use crate::{ConnState, Registry, Session, process_command};
use port4k_core::Username;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{Mutex, mpsc};

/// Small context passed around so helpers don't need tons of args
struct ConnCtx {
    registry: Arc<Registry>,
    sess: Arc<Mutex<Session>>,
    lua_tx: mpsc::Sender<LuaJob>,
}

/// Public entrypoint, now minimal and readable
pub async fn handle_connection(
    stream: TcpStream,
    registry: Arc<Registry>,
    banner: &str,
    entry: &str,
    lua_tx: mpsc::Sender<LuaJob>,
) -> anyhow::Result<()> {
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);

    let ctx = ConnCtx {
        registry,
        sess: Arc::new(Mutex::new(Session::default())),
        lua_tx,
    };

    let mut editor = LineEditor::new("LE> ");
    let mut telnet = TelnetMachine::new();

    start_session(&mut w, &mut telnet, banner, entry, &editor).await?;

    read_loop(&mut reader, &mut w, &mut telnet, &mut editor, &ctx).await?;

    cleanup(&ctx).await;
    Ok(())
}

async fn start_session(
    w: &mut OwnedWriteHalf,
    telnet: &mut TelnetMachine,
    banner: &str,
    entry: &str,
    editor: &LineEditor,
) -> anyhow::Result<()> {
    // Telnet option negotiation: character-at-a-time + SGA + (server) echo + NAWS
    telnet.start_negotiation(w).await?;

    // Welcome text
    if !banner.is_empty() {
        for line in banner.lines() {
            w.write_all(line.as_bytes()).await?;
            w.write_all(b"\r\n").await?;
        }
    }
    if !entry.is_empty() {
        for line in entry.lines() {
            w.write_all(line.as_bytes()).await?;
            w.write_all(b"\r\n").await?;
        }
    }

    repaint_prompt(w, editor).await?;
    Ok(())
}

async fn read_loop(
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut OwnedWriteHalf,
    telnet: &mut TelnetMachine,
    editor: &mut LineEditor,
    ctx: &ConnCtx,
) -> anyhow::Result<()> {
    let mut one = [0u8; 1];

    loop {
        let n = reader.read(&mut one).await?;
        if n == 0 {
            break; // disconnect
        }

        if let Some(evt) = telnet.push(one[0], w).await? {
            match evt {
                TelnetIn::Data(b) => handle_data_byte(b, reader, w, telnet, editor, ctx).await?,
                TelnetIn::Naws { cols, rows } => handle_naws(cols, rows, ctx).await,
            }
        }
    }
    Ok(())
}

async fn cleanup(ctx: &ConnCtx) {
    if let Some(u) = ctx.sess.lock().await.name.clone() {
        ctx.registry.set_online(&u, false).await;
    }
}

async fn handle_data_byte(
    b: u8,
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut OwnedWriteHalf,
    telnet: &mut TelnetMachine,
    editor: &mut LineEditor,
    ctx: &ConnCtx,
) -> anyhow::Result<()> {
    match editor.handle_byte(b) {
        EditEvent::None => {}
        EditEvent::Redraw => {
            repaint_prompt(w, editor).await?;
        }
        EditEvent::Line(line) => {
            // Move to a fresh line before emitting any output
            w.write_all(b"\r\n").await?;
            let raw = line.trim();
            tracing::debug!(%raw, "received line");

            // Try login flow first; if handled, we just repaint
            if try_handle_login(raw, reader, w, telnet, ctx).await? == LoginOutcome::Handled {
                repaint_prompt(w, editor).await?;
                return Ok(());
            }

            // Otherwise, dispatch as a normal command
            dispatch_command(raw, w, ctx).await?;

            // Always repaint prompt after server output
            repaint_prompt(w, editor).await?;
        }
    }
    Ok(())
}

async fn handle_naws(cols: u16, rows: u16, _ctx: &ConnCtx) {
    dbg!(&cols, &rows);
    // let mut s = ctx.sess.lock().await;
    // s.tty_cols = Some(cols as usize);
    // s.tty_rows = Some(rows as usize);
}

async fn dispatch_command(raw: &str, w: &mut OwnedWriteHalf, ctx: &ConnCtx) -> anyhow::Result<()> {
    match process_command(raw, &ctx.registry, &ctx.sess, ctx.lua_tx.clone()).await {
        Ok(out) => {
            if !out.is_empty() {
                write_with_newline(w, out.as_bytes()).await?;
            }
        }
        Err(e) => {
            write_with_newline(w, format!("error: {e}").as_bytes()).await?;
        }
    }
    Ok(())
}

#[derive(PartialEq, Eq)]
enum LoginOutcome {
    Handled,
    NotHandled,
}

/// Return `Handled` if login flow consumed the command
async fn try_handle_login(
    raw: &str,
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut OwnedWriteHalf,
    telnet: &mut TelnetMachine,
    ctx: &ConnCtx,
) -> anyhow::Result<LoginOutcome> {
    // Only handle `login <username>` with exactly one arg here
    let Some(rest) = raw.strip_prefix("login ") else {
        return Ok(LoginOutcome::NotHandled);
    };
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.len() != 1 {
        return Ok(LoginOutcome::NotHandled);
    }

    if ctx.sess.lock().await.state == ConnState::LoggedIn {
        write_with_newline(w, b"Already logged in.").await?;
        return Ok(LoginOutcome::Handled);
    }

    let Some(user) = Username::parse(parts[0]) else {
        write_with_newline(w, b"Invalid username.").await?;
        return Ok(LoginOutcome::Handled);
    };

    if !ctx.registry.user_exists(&user).await {
        write_with_newline(w, b"No such user. Try `register <name> <password>`.").await?;
        return Ok(LoginOutcome::Handled);
    }

    w.write_all(b"Password: ").await?;
    w.flush().await?;
    let pw = read_secret_line(reader, w, telnet, ctx).await?;

    if pw.is_empty() {
        // Keep the old behavior: end the connection when empty password
        return Ok(LoginOutcome::Handled);
    }

    let password = pw.trim_matches(['\r', '\n']);
    if ctx.registry.db.verify_user(&user.0, password).await.unwrap_or(false) {
        {
            let mut s = ctx.sess.lock().await;
            s.name = Some(user.clone());
            s.state = ConnState::LoggedIn;
        }
        ctx.registry.set_online(&user, true).await;

        write_with_newline(w, format!("Welcome, {}! Type `look` or `help`.", user).as_bytes()).await?;
    } else {
        write_with_newline(w, b"Invalid credentials.").await?;
    }

    Ok(LoginOutcome::Handled)
}

async fn repaint_prompt(w: &mut OwnedWriteHalf, editor: &LineEditor) -> std::io::Result<()> {
    w.write_all(editor.repaint_line().as_bytes()).await?;
    w.flush().await
}

/// Write text, ensuring it ends in CRLF (good Telnet hygiene)
async fn write_with_newline(w: &mut OwnedWriteHalf, bytes: &[u8]) -> std::io::Result<()> {
    w.write_all(bytes).await?;
    if !bytes.ends_with(b"\r\n") || !bytes.ends_with(b"\n") {
        w.write_all(b"\r\n").await?;
    }
    Ok(())
}

/// Read a single line (CR or LF) from the Telnet stream WITHOUT echoing.
/// Backspace/Delete are handled locally. NAWS events update the session.
/// Returns the collected bytes as a String (ASCII/UTF-8 expected).
async fn read_secret_line(
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut OwnedWriteHalf,
    telnet: &mut TelnetMachine,
    _ctx: &ConnCtx,
) -> std::io::Result<String> {
    let mut out = Vec::<u8>::new();
    let mut one = [0u8; 1];

    loop {
        let n = reader.read(&mut one).await?;
        if n == 0 { break; } // disconnect

        if let Some(evt) = telnet.push(one[0], w).await? {
            match evt {
                TelnetIn::Data(b) => {
                    match b {
                        b'\r' | b'\n' => {
                            // Move to the next line visually, but DO NOT reveal password
                            w.write_all(b"\r\n").await?;
                            break;
                        }
                        0x7F | 0x08 => {
                            // Backspace/Delete: remove last byte if any; no echo of backspace either
                            out.pop();
                        }
                        _ => {
                            // Only accept printable ASCII; ignore others for secrets
                            if b >= 0x20 && b < 0x7F {
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