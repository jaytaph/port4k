use std::collections::HashMap;
use crate::lua::LuaJob;
use crate::input::readline::{EditEvent, LineEditor};
use crate::util::telnet::{TelnetIn, TelnetMachine};
use crate::{ConnState, Registry, Session, process_command, WorldMode};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use crate::commands::CommandResult;
use crate::db::models::account::Account;

/// Wrapper around OwnedWriteHalf that normalizes bare '\n' to "\r\n"
pub struct CRLFOwnedWriteHalf<'a> {
    pub inner: &'a mut OwnedWriteHalf,
}

impl<'a> CRLFOwnedWriteHalf<'a> {
    pub fn new(inner: &'a mut OwnedWriteHalf) -> Self {
        Self { inner }
    }

    /// Writes `buf`, normalizing bare '\n' to "\r\n". Existing "\r\n" are preserved.
    pub async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        let mut last = 0;

        for i in 0..buf.len() {
            if buf[i] == b'\n' {
                // flush everything up to this point (excluding \n)
                if i > last {
                    self.inner.write_all(&buf[last..i]).await?;
                }

                // check previous byte
                if i == 0 || buf[i - 1] != b'\r' {
                    self.inner.write_all(b"\r\n").await?;
                } else {
                    self.inner.write_all(b"\n").await?;
                }

                last = i + 1; // move past the \n
            }
        }

        // write remaining tail (if no newline at end)
        if last < buf.len() {
            self.inner.write_all(&buf[last..]).await?;
        }

        Ok(())
    }

    pub async fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush().await
    }
}

/// Small context passed around so helpers don't need tons of args
struct ConnCtx {
    registry: Arc<Registry>,
    sess: Arc<RwLock<Session>>,
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

    let ctx = Arc::new(ConnCtx {
        registry,
        sess: Arc::new(RwLock::new(Session::default())),
        lua_tx,
    });

    let mut editor = LineEditor::new("> ");
    let mut telnet = TelnetMachine::new();

    start_session(&mut w, &mut telnet, banner, entry, &mut editor, ctx.clone()).await?;

    read_loop(&mut reader, &mut w, &mut telnet, &mut editor, ctx.clone()).await?;

    cleanup(&ctx).await;
    Ok(())
}

async fn start_session(
    w: &mut OwnedWriteHalf,
    telnet: &mut TelnetMachine,
    banner: &str,
    entry: &str,
    editor: &mut LineEditor,
    ctx: Arc<ConnCtx>,
) -> anyhow::Result<()> {
    // Telnet option negotiation: character-at-a-time + SGA + (server) echo + NAWS
    telnet.start_negotiation(w).await?;

    let crlf_w = &mut CRLFOwnedWriteHalf::new(w);

    // Welcome text
    if !banner.is_empty() {
        for line in banner.lines() {
            crlf_w.write_all(line.as_bytes()).await?;
            crlf_w.write_all(b"\n").await?;
        }
    }
    if !entry.is_empty() {
        for line in entry.lines() {
            crlf_w.write_all(line.as_bytes()).await?;
            crlf_w.write_all(b"\n").await?;
        }
    }

    repaint_prompt(ctx, w, editor).await?;
    Ok(())
}

async fn read_loop(
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut OwnedWriteHalf,
    telnet: &mut TelnetMachine,
    editor: &mut LineEditor,
    ctx: Arc<ConnCtx>,
) -> anyhow::Result<()> {
    let mut one = [0u8; 1];

    loop {
        let n = reader.read(&mut one).await?;
        if n == 0 {
            break; // disconnect
        }

        if let Some(evt) = telnet.push(one[0], w).await? {
            match evt {
                TelnetIn::Data(b) => handle_data_byte(b, reader, w, telnet, editor, ctx.clone()).await?,
                TelnetIn::Naws { cols, rows } => handle_naws(cols, rows, ctx.clone()).await,
            }
        }
    }
    Ok(())
}

fn generate_prompt(ctx: Arc<ConnCtx>, prompt: &str) -> String {
    let mut vars = HashMap::new();
    vars.insert("world".to_string(), "World".to_string());
    vars.insert("room".to_string(), "Room".to_string());
    vars.insert("wall_time".to_string(), chrono::Local::now().format("%H:%M:%S").to_string());
    vars.insert("online_time".to_string(), chrono::Local::now().format("%H:%M:%S").to_string());
    vars.insert("online_users".to_string(), format!("{}", 123));

    if let Some(account) = ctx.sess.read().unwrap().account.clone() {
        vars.insert("name".to_string(), account.username);
        vars.insert("role".to_string(), account.role);
        vars.insert("xp".to_string(), format!("{}", account.xp));
        vars.insert("health".to_string(), format!("{}", account.health));
        vars.insert("coins".to_string(), format!("{}", account.coins));
    }
    if let Some(world) = ctx.sess.read().unwrap().world.as_ref() {
        match world {
            WorldMode::Live { room_id } => {
                vars.insert("world".to_string(), "Normal".to_string());
                vars.insert("room".to_string(), format!("{}", room_id.clone()));
            }
            WorldMode::Playtest { bp, .. } => {
                vars.insert("world".to_string(), format!("Playtest({})", bp));
                vars.insert("room".to_string(), "dummyroom".to_string());
            }
        }
    }

    let mut out = prompt.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{}}}", k), &v);
    }

    out
}

async fn cleanup(ctx: &ConnCtx) {
    let account_opt = {
        let sess = ctx.sess.read().unwrap();
        sess.account.clone()
    };

    if let Some(a) = account_opt {
        ctx.registry.set_online(&a, false).await;
    }
}

async fn handle_data_byte(
    b: u8,
    reader: &mut BufReader<OwnedReadHalf>,
    w: &mut OwnedWriteHalf,
    telnet: &mut TelnetMachine,
    editor: &mut LineEditor,
    ctx: Arc<ConnCtx>,
) -> anyhow::Result<()> {
    match editor.handle_byte(b) {
        EditEvent::None => {}
        EditEvent::Redraw => {
            repaint_prompt(ctx.clone(), w, editor).await?;
        }
        EditEvent::Line(line) => {
            // Move to a fresh line before emitting any output
            w.write_all(b"\r\n").await?;
            let raw = line.trim();
            tracing::debug!(%raw, "received line");

            // Try login flow first; if handled, we just repaint
            if try_handle_login(raw, reader, w, telnet, ctx.clone()).await? == LoginOutcome::Handled {
                repaint_prompt(ctx.clone(), w, editor).await?;
                return Ok(());
            }

            // Otherwise, dispatch as a normal command
            dispatch_command(raw, w, ctx.clone()).await?;

            // Always repaint prompt after server output
            repaint_prompt(ctx.clone(), w, editor).await?;
        }
    }
    Ok(())
}

async fn handle_naws(cols: u16, rows: u16, _ctx: Arc<ConnCtx>) {
    dbg!(&cols, &rows);
    // let mut s = ctx.sess.lock().await;
    // s.tty_cols = Some(cols as usize);
    // s.tty_rows = Some(rows as usize);
}

async fn dispatch_command(raw: &str, w: &mut OwnedWriteHalf, ctx: Arc<ConnCtx>) -> anyhow::Result<()> {
    match process_command(raw, ctx.registry.clone(), ctx.sess.clone(), ctx.lua_tx.clone()).await {
        Ok(CommandResult::Success(out))     => {
            if !out.is_empty() {
                write_with_newline(w, out.as_bytes()).await?;
            }
        }
        Ok(CommandResult::Failure(out))     => {
            if !out.is_empty() {
                write_with_newline(w, format!("error: {out}").as_bytes()).await?;
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
    ctx: Arc<ConnCtx>,
) -> anyhow::Result<LoginOutcome> {
    // Only handle `login <username>` with exactly one arg here
    let Some(rest) = raw.strip_prefix("login ") else {
        return Ok(LoginOutcome::NotHandled);
    };
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.len() != 1 {
        return Ok(LoginOutcome::NotHandled);
    }

    if ctx.sess.read().unwrap().state == ConnState::LoggedIn {
        write_with_newline(w, b"Already logged in.").await?;
        return Ok(LoginOutcome::Handled);
    }

    let username = parts[0];

    if Account::validate_username(username).is_err() {
        write_with_newline(w, b"Invalid username.").await?;
        return Ok(LoginOutcome::Handled);
    };

    if !ctx.registry.services.account.exists(&username).await? {
        write_with_newline(w, b"No such user. Try `register <name> <password>`.").await?;
        return Ok(LoginOutcome::Handled);
    }

    w.write_all(b"Password: ").await?;
    w.flush().await?;
    let pw = read_secret_line(reader, w, telnet, ctx.clone()).await?;

    if pw.is_empty() {
        // Keep the old behavior: end the connection when empty password
        return Ok(LoginOutcome::Handled);
    }

    let password = pw.trim_matches(['\r', '\n']);
    if !ctx.registry.services.auth.authenticate(&username, password).await.unwrap_or(false) {
        write_with_newline(w, b"Invalid credentials.").await?;
        return Ok(LoginOutcome::Handled);
    }


    // All is ok
    let Some(account) = ctx.registry.repos.account.get_by_username(&username).await? else {
        write_with_newline(w, b"Account retrieval error.").await?;
        return Ok(LoginOutcome::Handled);
    };

    {
        let mut s = ctx.sess.write().unwrap();
        s.account = Some(account.clone());
        s.state = ConnState::LoggedIn;
    }
    ctx.registry.set_online(&account, true).await;

    write_with_newline(w, format!("Welcome, {}! Type `look` or `help`.", account.username).as_bytes()).await?;
    Ok(LoginOutcome::Handled)
}

async fn repaint_prompt(ctx: Arc<ConnCtx>, w: &mut OwnedWriteHalf, editor: &mut LineEditor) -> std::io::Result<()> {
    let mut w = CRLFOwnedWriteHalf::new(w);

    let prompt = generate_prompt(ctx, &"{user} [{world}:{room}] @ {wall_time} > ");
    editor.set_prompt(&prompt);

    w.write_all(editor.repaint_line().as_bytes()).await?;
    w.flush().await
}

/// Write text, ensuring it ends in CRLF (good Telnet hygiene)
async fn write_with_newline(w: &mut OwnedWriteHalf, bytes: &[u8]) -> std::io::Result<()> {
    let mut w = CRLFOwnedWriteHalf::new(w);

    w.write_all(bytes).await?;
    if !bytes.ends_with(b"\n") || !bytes.ends_with(b"\n") {
        w.write_all(b"\n").await?;
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
    _ctx: Arc<ConnCtx>,
) -> std::io::Result<String> {
    let mut w_crlf = CRLFOwnedWriteHalf::new(w);

    let mut out = Vec::<u8>::new();
    let mut one = [0u8; 1];

    loop {
        let n = reader.read(&mut one).await?;
        if n == 0 {
            break;
        } // disconnect

        if let Some(evt) = telnet.push(one[0], w_crlf.inner).await? {
            match evt {
                TelnetIn::Data(b) => {
                    match b {
                        b'\r' | b'\n' => {
                            // Move to the next line visually, but DO NOT reveal password
                            w_crlf.write_all(b"\n").await?;
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
