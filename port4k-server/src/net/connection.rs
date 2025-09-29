use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};

use crate::commands::process_command;
use crate::lua::LuaJob;
use crate::scripting::editor::process_editor_line;
use crate::state::registry::Registry;
use crate::state::session::{ConnState, Session};
use crate::util::telnet::{read_telnet_line, telnet_echo_off, telnet_echo_on};
use port4k_core::Username;

pub async fn handle_connection(
    stream: TcpStream,
    registry: Arc<Registry>,
    banner: &str,
    entry: &str,
    lua_tx: mpsc::Sender<LuaJob>,
) -> anyhow::Result<()> {
    // handle_connection()
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);

    w.write_all(banner.as_bytes()).await?;
    w.write_all(entry.as_bytes()).await?;
    w.write_all(b"> ").await?;
    w.flush().await?;

    let sess = Arc::new(Mutex::new(Session::default()));
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }

        let raw = line.trim_matches(['\r', '\n']).trim();
        tracing::debug!(%raw, "received line");

        // editor branch first
        if sess.lock().await.editor.is_some() {
            let out = process_editor_line(raw, &registry, &sess).await?;
            if !out.is_empty() {
                w.write_all(out.as_bytes()).await?;
            }
            if sess.lock().await.editor.is_none() {
                w.write_all(b"> ").await?;
                w.flush().await?;
            }
            continue;
        }

        // telnet two-step login
        if let Some(rest) = raw.strip_prefix("login ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 1 {
                if sess.lock().await.state == ConnState::LoggedIn {
                    w.write_all(b"Already logged in.\n").await?;
                } else if let Some(user) = Username::parse(parts[0]) {
                    if !registry.user_exists(&user).await {
                        w.write_all(b"No such user. Try `register <name> <password>`.\n")
                            .await?;
                    } else {
                        telnet_echo_off(&mut w).await?;
                        w.write_all(b"Password: ").await?;
                        w.flush().await?;

                        let pw = read_telnet_line(&mut reader).await?;
                        if pw.is_empty() {
                            telnet_echo_on(&mut w).await?;
                            return Ok(());
                        }
                        telnet_echo_on(&mut w).await?;
                        let password = pw.trim_matches(['\r', '\n']);

                        if registry
                            .db
                            .verify_user(&user.0, password)
                            .await
                            .unwrap_or(false)
                        {
                            let mut s = sess.lock().await;
                            s.name = Some(user.clone());
                            s.state = ConnState::LoggedIn;
                            registry.set_online(&user, true).await;
                            w.write_all(
                                format!("\nWelcome, {}! Type `look` or `help`.\n", user).as_bytes(),
                            )
                            .await?;
                        } else {
                            w.write_all(b"\nInvalid credentials.\n").await?;
                        }
                    }
                } else {
                    w.write_all(b"Invalid username.\n").await?;
                }
                w.write_all(b"> ").await?;
                w.flush().await?;
                continue;
            }
        }

        // normal commands
        match process_command(raw, &registry, &sess, lua_tx.clone()).await {
            Ok(mut out) => {
                if !out.is_empty() {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    w.write_all(out.as_bytes()).await?;
                }
            }
            Err(e) => {
                w.write_all(format!("error: {e}\n").as_bytes()).await?;
            }
        }

        w.write_all(b"> ").await?;
        w.flush().await?;
    }

    if let Some(u) = sess.lock().await.name.clone() {
        registry.set_online(&u, false).await;
    }
    Ok(())
}
