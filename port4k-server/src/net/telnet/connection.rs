use crate::commands::CmdCtx;
use crate::error::AppResult;
use crate::input::readline::{EditEvent, LineEditor};
use crate::lua::{LuaJob, LuaResult};
use crate::models::account::Account;
use crate::net::AppCtx;
use crate::net::output::OutputHandle;
use crate::util::telnet::{TelnetIn, TelnetMachine};
use crate::{ConnState, Registry, Session, process_command};
use parking_lot::RwLock;
use std::collections::HashSet;
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
            let raw = line.trim();

            let in_repl = sess.read().in_lua_repl;

            if in_repl {
                handle_repl_input(raw, ctx.clone(), sess.clone()).await?;
                // ctx.output.line("\n\n").await;
                update_prompt(sess.clone(), ctx.output.clone(), editor).await;
                return Ok(());
            }

            // // Move to a fresh line before emitting any output
            ctx.output.line("\n\n").await;

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
        ctx.output
            .system("No such user. Try `register <name> <password>`.")
            .await;
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

    ctx.output
        .system(format!("Welcome, {}! Type `look` or `help`.", account.username))
        .await;
    Ok(LoginOutcome::Handled)
}

fn generate_prompt(sess: Arc<RwLock<Session>>, prompt: &str) -> String {
    let s = sess.read();
    if s.in_lua_repl {
        return "lua> ".to_string();
    }

    prompt.to_string()

    // // No room view vars in prompt generation
    // let vars = RenderVars::new(sess.clone(), None);
    // render_template(prompt, &vars, 80)
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
        {
            let mut s = sess.write();
            s.in_lua_repl = false;
        }

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

    let job = LuaJob::ReplEval {
        output_handle: ctx.output.clone(),
        account: sess.read().account.clone().unwrap().clone(),
        cursor: Box::new(sess.read().cursor.clone().unwrap().clone()),
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

fn format_lua_value(value: &mlua::Value) -> String {
    format_lua_value_impl(value, 0, &mut HashSet::new())
}

fn format_lua_value_impl(value: &mlua::Value, indent: usize, seen: &mut HashSet<usize>) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => n.to_string(),
        mlua::Value::String(s) => format!("\"{}\"", String::from_utf8_lossy(s.as_bytes().as_ref())),
        mlua::Value::Table(t) => print_lua_table(t, indent, seen),
        mlua::Value::Function(_) => "<function>".to_string(),
        mlua::Value::Thread(_) => "<thread>".to_string(),
        mlua::Value::UserData(_) => "<userdata>".to_string(),
        mlua::Value::LightUserData(_) => "<lightuserdata>".to_string(),
        mlua::Value::Error(e) => format!("error: {}", e),
        _ => "<unknown>".to_string(),
    }
}

fn print_lua_table(table: &mlua::Table, indent: usize, seen: &mut HashSet<usize>) -> String {
    // Get a unique identifier for this table to detect cycles
    let table_ptr = table.to_pointer() as usize;

    // Check for circular reference
    if seen.contains(&table_ptr) {
        return "<circular reference>".to_string();
    }

    seen.insert(table_ptr);

    let mut result = String::from("{\n");
    let indent_str = "  ".repeat(indent + 1);

    // Try to get all pairs - if it fails, just show <table>
    let pairs = match table.pairs::<mlua::Value, mlua::Value>().collect::<Result<Vec<_>, _>>() {
        Ok(pairs) => pairs,
        Err(_) => {
            seen.remove(&table_ptr);
            return "<table>".to_string();
        }
    };

    // Separate numeric indices (array part) from other keys
    let mut array_items: Vec<(i64, mlua::Value)> = Vec::new();
    let mut hash_items: Vec<(mlua::Value, mlua::Value)> = Vec::new();

    for (key, value) in pairs {
        if let mlua::Value::Integer(i) = key {
            array_items.push((i, value));
        } else {
            hash_items.push((key, value));
        }
    }

    // Sort array items by index
    array_items.sort_by_key(|(i, _)| *i);

    // Print array part (consecutive integers starting from 1)
    let mut last_idx = 0;
    for (idx, value) in array_items {
        // Check if indices are consecutive
        if idx == last_idx + 1 {
            let formatted_value = format_lua_value_impl(&value, indent + 1, seen);
            result.push_str(&format!("{}{},\n", indent_str, formatted_value));
            last_idx = idx;
        } else {
            // Non-consecutive, treat as hash key
            let formatted_value = format_lua_value_impl(&value, indent + 1, seen);
            result.push_str(&format!("{}[{}] = {},\n", indent_str, idx, formatted_value));
        }
    }

    // Print hash part
    for (key, value) in hash_items {
        let formatted_key = format_lua_key(&key);
        let formatted_value = format_lua_value_impl(&value, indent + 1, seen);
        result.push_str(&format!("{}{} = {},\n", indent_str, formatted_key, formatted_value));
    }

    // Remove trailing comma and newline if present
    if result.ends_with(",\n") {
        result.truncate(result.len() - 2);
        result.push('\n');
    }

    let close_indent = "  ".repeat(indent);
    result.push_str(&format!("{}}}", close_indent));

    seen.remove(&table_ptr);

    result
}

fn format_lua_key(key: &mlua::Value) -> String {
    match key {
        mlua::Value::String(s) => {
            let bytes = s.as_bytes();
            let key_str = String::from_utf8_lossy(bytes.as_ref());
            // Check if it's a valid identifier (no need for brackets)
            if is_valid_lua_identifier(&key_str) {
                key_str.into_owned()
            } else {
                format!("[\"{}\"]", key_str)
            }
        }
        mlua::Value::Integer(i) => format!("[{}]", i),
        mlua::Value::Number(n) => format!("[{}]", n),
        mlua::Value::Boolean(b) => format!("[{}]", b),
        _ => "[<complex key>]".to_string(),
    }
}

fn is_valid_lua_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // Check if it's a Lua keyword
    const LUA_KEYWORDS: &[&str] = &[
        "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local", "nil", "not",
        "or", "repeat", "return", "then", "true", "until", "while",
    ];

    if LUA_KEYWORDS.contains(&s) {
        return false;
    }

    // Check if first char is letter or underscore
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }

    // Check remaining chars are alphanumeric or underscore
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
