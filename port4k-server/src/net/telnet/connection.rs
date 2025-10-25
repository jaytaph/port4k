use crate::commands::CmdCtx;
use crate::error::AppResult;
use crate::input::readline::{EditEvent, LineEditor};
use crate::models::account::Account;
use crate::net::AppCtx;
use crate::renderer::{RenderVars, render_template};
use crate::util::telnet::{TelnetIn, TelnetMachine};
use crate::{ConnState, Registry, Session, process_command};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use crate::net::output::OutputHandle;

pub async fn handle_connection(
    read_half: OwnedReadHalf,
    ctx: Arc<AppCtx>,
    telnet: &mut TelnetMachine,
    sess: Arc<RwLock<Session>>
) -> AppResult<()> {
    let mut reader = BufReader::new(read_half);
    let mut editor = LineEditor::new("> ");

    update_prompt(sess.clone(), ctx.output.clone(), &mut editor).await;

    read_loop(&mut reader, telnet, &mut editor, ctx.clone(), sess.clone()).await?;
    cleanup(sess.clone(), ctx.registry.clone()).await;

    Ok(())
}

// //noinspection RsExternalLinter
// async fn start_session<W: AsyncWrite + Unpin>(
//     w: &mut SlowWriter<W>,
//     telnet: &mut TelnetMachine,
//     editor: &mut LineEditor,
//     sess: Arc<RwLock<Session>>,
// ) -> AppResult<()> {
//     // Telnet option negotiation: character-at-a-time + SGA + (server) echo + NAWS
//     telnet.start_negotiation(w).await?;
//
//     // Welcome text
//     if !BANNER.is_empty() {
//         for line in BANNER.lines() {
//             w.write_all(line.as_bytes()).await?;
//             w.write_all(b"\n").await?;
//         }
//     }
//     if !ENTRY.is_empty() {
//         for line in ENTRY.lines() {
//             w.write_all(line.as_bytes()).await?;
//             w.write_all(b"\n").await?;
//         }
//     }
//
//     repaint_prompt(sess.clone(), w, editor, true).await?;
//     Ok(())
// }

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
                TelnetIn::Data(b) => {
                    handle_data_byte(b, reader, telnet, editor, sess.clone(), ctx.clone()).await?
                },
                TelnetIn::Naws { cols, rows } => {
                    handle_naws(cols, rows, sess.clone()).await
                },
            }
        }
    }
    Ok(())
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

async fn handle_data_byte(
    b: u8,
    reader: &mut BufReader<OwnedReadHalf>,
    telnet: &mut TelnetMachine,
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
            // // Move to a fresh line before emitting any output
            // w.write_all(b"\r\n").await?;
            // w.write_all(b"\r\n").await?;
            let raw = line.trim();
            tracing::debug!(%raw, "received line");

            // Try login flow first; if handled, we just repaint
            if try_handle_login(raw, reader, telnet, ctx.clone(), sess.clone()).await? == LoginOutcome::Handled {
                update_prompt(sess.clone(), ctx.output.clone(), editor).await;
                return Ok(());
            }

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
    s.tty_cols = Some(cols as usize);
    s.tty_rows = Some(rows as usize);
}

async fn dispatch_command(raw: &str, ctx: Arc<AppCtx>, sess: Arc<RwLock<Session>>) -> AppResult<()> {
    let cmd_ctx = Arc::new(CmdCtx {
        registry: ctx.registry.clone(),
        output: ctx.output.clone(),
        lua_tx: ctx.lua_tx.clone(),
        sess: sess.clone(),
    });

    _ = process_command(raw, cmd_ctx.clone()).await;
    Ok(())
}

// async fn output_error<W: AsyncWrite + Unpin>(w: &mut W, out: CommandOutput) {
//     let error_template = r#"
// {c:bright_yellow:bright_red} An error occurred while processing your command.{c}
//
// {v:messages}
//     "#;
//
//     let vars = RenderVars::default().with("messages", out.messages().join("").as_str());
//     let rendered_out = render_template(error_template, &vars, 80);
//
//     let _ = write_with_newline(w, rendered_out.as_bytes()).await;
// }

// async fn output_success<W: AsyncWrite + Unpin>(w: &mut W, out: CommandOutput) {
//     let _ = write_with_newline(w, b"\n").await;
//     for msg in out.messages() {
//         let _ = write_with_newline(w, msg.as_bytes()).await;
//         // let _ = write_with_newline(w, b"\n").await;
//     }
//     let _ = write_with_newline(w, b"\n").await;
// }

#[derive(PartialEq, Eq)]
enum LoginOutcome {
    Handled,
    NotHandled,
}

/// Return `Handled` if login flow consumed the command
async fn try_handle_login(
    raw: &str,
    reader: &mut BufReader<OwnedReadHalf>,
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
        ctx.output.system("Already logged in.").await;
        return Ok(LoginOutcome::Handled);
    }

    let username = parts[0];

    if Account::validate_username(username).is_err() {
        ctx.output.system("Invalid username.").await;
        return Ok(LoginOutcome::Handled);
    };

    if !ctx.registry.services.account.exists(username).await? {
        ctx.output.system("No such user. Try `register <name> <password>`.").await;
        return Ok(LoginOutcome::Handled);
    }


    // We need special handling here to avoid echoing the password
    ctx.output.system("Password: ").await;
    let pw = read_secret_line(reader, telnet, ctx.clone()).await?;

    if pw.is_empty() {
        // Keep the old behavior: end the connection when empty password
        return Ok(LoginOutcome::Handled);
    }

    let password = pw.trim_matches(['\r', '\n']);
    let account = ctx.registry.services.auth.authenticate(username, password).await?;

    {
        let mut s = sess.write();
        s.account = Some(account.clone());
        s.state = ConnState::LoggedIn;
    }
    ctx.registry.set_online(&account, true).await;

    ctx.output.system(format!("Welcome, {}! Type `look` or `help`.", account.username)).await;
    Ok(LoginOutcome::Handled)
}

fn generate_prompt(sess: Arc<RwLock<Session>>, prompt: &str) -> String {
    // No roomview vars in prompt generation
    let vars = RenderVars::new(sess.clone(), None);
    render_template(prompt, &vars, 80)
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
