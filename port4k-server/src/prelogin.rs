use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::db::Db;
use crate::telnet::telnet_echo_on;
use crate::telnet::{read_telnet_line, telnet_echo_off};
use port4k_core::Username;

#[derive(Debug)]
pub struct Editor {
    bp: String,
    room: String,
    event: String,
    buf: String,
}

#[derive(Debug, Clone)]
pub enum WorldMode {
    Live {
        room_id: i64,
    },
    Playtest {
        bp: String,
        room: String,
        prev_room_id: Option<i64>,
    },
}

#[derive(Debug)]
pub struct Registry {
    pub db: Db,
    pub online: RwLock<std::collections::BTreeSet<String>>,
}

impl Registry {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            online: RwLock::new(std::collections::BTreeSet::new()),
        }
    }

    pub async fn set_online(&self, name: &Username, online: bool) {
        let mut g = self.online.write().await;
        if online {
            g.insert(name.0.clone());
        } else {
            g.remove(&name.0);
        }
    }

    pub async fn who(&self) -> Vec<String> {
        self.online.read().await.iter().cloned().collect()
    }

    pub async fn user_exists(&self, name: &Username) -> bool {
        self.db.user_exists(&name.0).await.unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    PreLogin,
    LoggedIn,
}

#[derive(Debug)]
pub struct Session {
    pub name: Option<Username>,
    pub state: ConnState,
    pub world: Option<WorldMode>,
    pub editor: Option<Editor>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            name: None,
            state: ConnState::PreLogin,
            world: None,
            editor: None,
        }
    }
}

pub async fn handle_connection(
    stream: TcpStream,
    registry: Arc<Registry>,
    banner: &str,
    entry: &str,
    lua_tx: mpsc::Sender<crate::lua::LuaJob>,
) -> anyhow::Result<()> {
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);

    // Print banner and entry help
    w.write_all(banner.as_bytes()).await?;
    w.write_all(entry.as_bytes()).await?;
    w.write_all(b"> ").await?;
    w.flush().await?;

    let sess = Arc::new(Mutex::new(Session::default()));

    let mut line = String::new();
    loop {
        line.clear(); // important: read_line appends
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }

        let raw = line.trim_matches(['\r', '\n']).trim();
        tracing::debug!(%raw, "received line");

        // If in editor mode, process and keep the connection alive
        if sess.lock().await.editor.is_some() {
            let out = process_editor_line(raw, &registry, &sess).await?;
            if !out.is_empty() {
                w.write_all(out.as_bytes()).await?;
            }

            // If the editor just ended (.end), show the normal prompt again
            if sess.lock().await.editor.is_none() {
                w.write_all(b"> ").await?;
                w.flush().await?;
            }
            continue;
        }

        // Special-case: telnet two-step login ("login <name>" → prompt for password)
        if let Some(rest) = raw.strip_prefix("login ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 1 {
                // already logged in?
                if sess.lock().await.state == ConnState::LoggedIn {
                    w.write_all(b"Already logged in.\n").await?;
                } else {
                    // validate user and prompt for password with echo off
                    if let Some(user) = Username::parse(parts[0]) {
                        if !registry.user_exists(&user).await {
                            w.write_all(b"No such user. Try `register <name> <password>`.\n")
                                .await?;
                        } else {
                            // turn echo off, then prompt
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
                                    format!("\nWelcome, {}! Type `look` or `help`.\n", user)
                                        .as_bytes(),
                                )
                                .await?;
                            } else {
                                w.write_all(b"\nInvalid credentials.\n").await?;
                            }
                        }
                    } else {
                        w.write_all(b"Invalid username.\n").await?;
                    }
                }
                // re-print prompt and continue next iteration
                w.write_all(b"> ").await?;
                w.flush().await?;
                continue;
            }
        }

        // Normal command path
        let response = process_command(raw, &registry, &sess, lua_tx.clone()).await;
        match response {
            Ok(mut out) => {
                if !out.is_empty() {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    w.write_all(out.as_bytes()).await?;
                }
            }
            Err(e) => {
                use tokio::io::AsyncWriteExt;
                w.write_all(format!("error: {e}\n").as_bytes()).await?;
            }
        }

        w.write_all(b"> ").await?;
        w.flush().await?;
    }

    // On disconnect, mark offline if logged in
    let name = sess.lock().await.name.clone();
    if let Some(u) = name {
        registry.set_online(&u, false).await;
    }
    Ok(())
}

pub(crate) async fn process_command(
    cmd: &str,
    registry: &Arc<Registry>,
    sess: &Arc<Mutex<Session>>,
    lua_tx: mpsc::Sender<crate::lua::LuaJob>,
) -> anyhow::Result<String> {
    if cmd.is_empty() {
        return Ok(String::new());
    }

    let mut it = cmd.split_whitespace();
    let Some(verb) = it.next() else {
        return Ok(String::new());
    };

    match verb.to_ascii_lowercase().as_str() {
        "help" => Ok(help_text()),
        "quit" | "exit" => Ok("Goodbye!\n".to_string()),
        "who" => {
            let list = registry.who().await;
            if list.is_empty() {
                Ok("No one is online.\n".into())
            } else {
                Ok(format!("Online ({}): {}\n", list.len(), list.join(", ")))
            }
        }
        "register" => {
            let args = it.collect::<Vec<_>>();
            if args.len() < 2 {
                return Ok("Usage: register <name> <password>\n".into());
            }
            let name = args[0];
            let pass = args[1];
            let Some(u) = Username::parse(name) else {
                return Ok("Invalid username.\n".into());
            };
            if registry.db.register_user(&u.0, pass).await? {
                Ok(format!(
                    "Account `{}` created. You can now `login {} <password>`.\n",
                    u, u
                ))
            } else {
                Ok("That name is taken.\n".into())
            }
        }
        "login" => {
            let args = it.collect::<Vec<_>>();
            if args.len() < 2 {
                return Ok("Usage: login <name> <password>\n".into());
            }
            let name = args[0];
            let pass = args[1];
            let Some(u) = Username::parse(name) else {
                return Ok("Invalid username.\n".into());
            };
            if registry.db.verify_user(&u.0, pass).await? {
                let (_char_id, loc) = registry.db.get_or_create_character(&u.0).await?;
                {
                    let mut s = sess.lock().await;
                    s.name = Some(u.clone());
                    s.state = ConnState::LoggedIn;
                    s.world = Some(WorldMode::Live { room_id: loc });
                }
                registry.set_online(&u, true).await;
                let view = registry.db.room_view(loc).await?;
                Ok(format!("Welcome, {}!\n{}", u, view))
            } else {
                Ok("Invalid credentials.\n".into())
            }
        }
        "look" => {
            let s = sess.lock().await;
            if s.state != ConnState::LoggedIn {
                return Ok("You must `login` first.\n".into());
            }
            match &s.world {
                Some(WorldMode::Live { room_id }) => {
                    let view = registry.db.room_view(*room_id).await?;
                    Ok(view)
                }
                Some(WorldMode::Playtest { bp, room, .. }) => {
                    match registry.db.bp_room_view(bp, room).await? {
                        Some(view) => Ok(view),
                        None => Ok("[playtest] This room does not exist.\n".into()),
                    }
                }
                None => Ok("You are nowhere.\n".into()),
            }
        }
        "go" => {
            let args = it.collect::<Vec<_>>();
            if args.is_empty() {
                return Ok("Usage: go <direction>\n".into());
            }
            let dir = args[0].to_ascii_lowercase();

            // 1) Snapshot user + world, then drop the lock
            let (username, world) = {
                let s = sess.lock().await;
                if s.state != ConnState::LoggedIn {
                    return Ok("You must `login` first.\n".into());
                }
                let username = s.name.as_ref().unwrap().0.clone();
                let world = s.world.clone(); // make sure WorldMode derives Clone
                (username, world)
            };

            // 2) Do DB work without holding the lock; 3) Re-lock only to update state
            match world {
                Some(WorldMode::Live { .. }) => {
                    match registry.db.move_character(&username, &dir).await? {
                        Some(new_room) => {
                            {
                                // update session
                                let mut s = sess.lock().await;
                                if let Some(WorldMode::Live { room_id }) = &mut s.world {
                                    *room_id = new_room;
                                }
                            }
                            let view = registry.db.room_view(new_room).await?;
                            Ok(view)
                        }
                        None => Ok("You can't go that way.\n".into()),
                    }
                }
                Some(WorldMode::Playtest { bp, room, .. }) => {
                    match registry.db.bp_move(&bp, &room, &dir).await? {
                        Some(next) => {
                            {
                                // update session
                                let mut s = sess.lock().await;
                                if let Some(WorldMode::Playtest { room, .. }) = &mut s.world {
                                    *room = next.clone();
                                }
                            }
                            let extra = crate::lua::run_on_enter_playtest(
                                &registry.db,
                                &bp,
                                &next,
                                &username,
                            )
                            .await?
                            .unwrap_or_default();
                            let view = registry
                                .db
                                .bp_room_view(&bp, &next)
                                .await?
                                .unwrap_or_else(|| "[playtest] room missing\n".into());
                            Ok(format!("{view}{extra}"))
                        }
                        None => Ok("You can't go that way (playtest).\n".into()),
                    }
                }
                None => Ok("You are nowhere.\n".into()),
            }
        }
        "take" => {
            let args = it.collect::<Vec<_>>();
            if args.is_empty() {
                return Ok("Usage: take coin [N]\n".into());
            }
            let what = args[0].to_ascii_lowercase();
            if what != "coin" && what != "coins" {
                return Ok("You can take: coin\n".into());
            }

            // default 1 coin; allow "take coin 3"
            let want: i32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

            // Snapshot session state once (avoid multiple locks)
            let (user, loc) = {
                let s = sess.lock().await;
                let user = match &s.name {
                    Some(u) => u.clone(),
                    None => return Ok("You must `login` first.\n".into()),
                };
                match &s.world {
                    Some(WorldMode::Live { room_id }) => (user, *room_id),
                    Some(WorldMode::Playtest { .. }) => {
                        return Ok(
                            "[playtest] Coins aren’t available in playtest instances.\n".into()
                        );
                    }
                    None => return Ok("You are nowhere.\n".into()),
                }
            };

            let got = registry.db.pickup_coins(&user.0, loc, want).await?;
            if got == 0 {
                Ok("There are no coins to pick up.\n".into())
            } else {
                Ok(format!("You pick up {got} coin(s).\n"))
            }
        }
        "balance" => {
            let s_name = { sess.lock().await.name.clone() };
            if s_name.is_none() {
                return Ok("You must `login` first.\n".into());
            }
            let user = s_name.unwrap();
            let bal = registry.db.account_balance(&user.0).await?;
            Ok(format!("Your balance: {bal} coin(s).\n"))
        }
        "@bp" => {
            let rest = cmd.strip_prefix("@bp").unwrap().trim();
            let parts = split_args_quoted(rest);
            if parts.is_empty() {
                return Ok("Usage:\n  @bp new <bp> \"Title\"\n  @bp room add <bp>:<room> \"Title\" \"Body\"\n  @bp exit add <bp>:<from> <dir> <bp>:<to>\n  @bp entry <bp>:<room>\n  @bp submit <bp>\n".into());
            }

            match parts[0].as_str() {
                "new" if parts.len() >= 3 => {
                    let bp = &parts[1];
                    let title = &parts[2];
                    let owner = sess.lock().await.name.as_ref().ok_or_else(|| anyhow::anyhow!("login required"))?.0.clone();
                    if registry.db.bp_new(bp, title, &owner).await? {
                        Ok(format!("[bp] created `{}`: {}\n", bp, title))
                    } else { Ok("[bp] already exists.\n".into()) }
                }
                "room" if parts.len() >= 5 && parts[1] == "add" => {
                    let (bp, room) = parse_bp_room_key(&parts[2]).ok_or_else(|| anyhow::anyhow!("room key must be <bp>:<room>"))?;
                    let title = &parts[3];
                    let body = &parts[4];
                    if registry.db.bp_room_add(&bp, &room, title, body).await? {
                        Ok(format!("[bp] room {}:{} added.\n", bp, room))
                    } else { Ok("[bp] room already exists.\n".into()) }
                }
                "exit" if parts.len() >= 5 && parts[1] == "add" => {
                    let (bp1, from) = parse_bp_room_key(&parts[2]).ok_or_else(|| anyhow::anyhow!("from must be <bp>:<room>"))?;
                    let dir = parts[3].to_ascii_lowercase();
                    let (bp2, to) = parse_bp_room_key(&parts[4]).ok_or_else(|| anyhow::anyhow!("to must be <bp>:<room>"))?;
                    if bp1 != bp2 { return Ok("[bp] exits must stay within the same blueprint.\n".into()); }
                    if registry.db.bp_exit_add(&bp1, &from, &dir, &to).await? {
                        Ok(format!("[bp] exit {}:{} --{}--> {} added.\n", bp1, from, dir, to))
                    } else { Ok("[bp] exit already exists.\n".into()) }
                }
                "entry" if parts.len() >= 2 => {
                    let (bp, room) = parse_bp_room_key(&parts[1]).ok_or_else(|| anyhow::anyhow!("use <bp>:<room>"))?;
                    if registry.db.bp_set_entry(&bp, &room).await? {
                        Ok(format!("[bp] entry set: {}:{}\n", bp, room))
                    } else { Ok("[bp] blueprint not found.\n".into()) }
                }
                "submit" if parts.len() >= 2 => {
                    // Just mark as submitted (moderation flow can be added later)
                    let client = registry.db.pool.get().await?;
                    let n = client.execute("UPDATE blueprints SET status='submitted' WHERE key=$1", &[&parts[1]]).await?;
                    if n == 1 { Ok("[bp] submitted for review.\n".into()) } else { Ok("[bp] not found.\n".into()) }
                }
                _ => Ok("Usage:\n  @bp new <bp> \"Title\"\n  @bp room add <bp>:<room> \"Title\" \"Body\"\n  @bp exit add <bp>:<from> <dir> <bp>:<to>\n  @bp entry <bp>:<room>\n  @bp submit <bp>\n".into()),
            }
        }
        "@playtest" => {
            let rest = cmd.strip_prefix("@playtest").unwrap().trim();
            if rest.eq_ignore_ascii_case("stop") {
                let mut s = sess.lock().await;
                match &mut s.world {
                    Some(WorldMode::Playtest { prev_room_id, .. }) => {
                        let room_id = prev_room_id
                            .take()
                            .ok_or_else(|| anyhow::anyhow!("no previous location"))?;
                        s.world = Some(WorldMode::Live { room_id });
                        let view = registry.db.room_view(room_id).await?;
                        Ok(format!("[playtest] exited.\n{view}"))
                    }
                    _ => Ok("[playtest] you are not in playtest.\n".into()),
                }
            } else {
                let bp = rest;
                let entry = registry
                    .db
                    .bp_entry(bp)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("blueprint has no entry room"))?;
                let mut s = sess.lock().await;
                if s.state != ConnState::LoggedIn {
                    return Ok("Login required.\n".into());
                }
                let prev = match &s.world {
                    Some(WorldMode::Live { room_id }) => Some(*room_id),
                    _ => None,
                };
                s.world = Some(WorldMode::Playtest {
                    bp: bp.to_string(),
                    room: entry.clone(),
                    prev_room_id: prev,
                });
                let view = registry
                    .db
                    .bp_room_view(bp, &entry)
                    .await?
                    .unwrap_or_else(|| "[playtest] empty room\n".into());
                Ok(format!(
                    "[playtest] entered `{}` at `{}`.\n{}",
                    bp, entry, view
                ))
            }
        }
        "@debug" => {
            let rest = cmd.strip_prefix("@debug").unwrap().trim();
            let sub = rest.split_whitespace().next().unwrap_or("");
            match sub {
                "where" => {
                    let s = sess.lock().await;
                    let user = s.name.as_ref().map(|u| u.0.as_str()).unwrap_or("<guest>");
                    let msg = match &s.world {
                        Some(WorldMode::Live { room_id }) => {
                            format!("[debug] user={user} world=Live room_id={}\n", room_id)
                        }
                        Some(WorldMode::Playtest { bp, room, .. }) => {
                            format!("[debug] user={user} world=Playtest {}:{}\n", bp, room)
                        }
                        None => format!("[debug] user={user} world=None\n"),
                    };
                    Ok(msg)
                }
                _ => Ok("Usage: @debug where\n".into()),
            }
        }
        "@script" => {
            let parts = split_args_quoted(cmd.trim_start_matches("@script").trim());
            if parts.is_empty() {
                return Ok("Usage:\n  @script edit <bp>:<room> <event>\n  @script publish <bp>:<room> <event>\nNotes:\n  End editor with a single line: .end\n  Events: on_command | on_enter | on_timer\n".into());
            }
            match parts[0].as_str() {
                "edit" if parts.len() >= 3 => {
                    let (bp, room) = parse_bp_room_key(&parts[1]).ok_or_else(|| anyhow::anyhow!("room must be <bp>:<room>"))?;
                    let event = parts[2].to_string();
                    // basic validation
                    let allowed = ["on_command","on_enter","on_timer"];
                    if !allowed.contains(&event.as_str()) {
                        return Ok("Event must be: on_command | on_enter | on_timer\n".into());
                    }
                    // start editor
                    {
                        let mut s = sess.lock().await;
                        if s.state != ConnState::LoggedIn { return Ok("Login required.\n".into()); }
                        s.editor = Some(Editor { bp, room, event, buf: String::new() });
                    }
                    Ok("[editor] Paste your Lua. End with a single line: .end\n".into())
                }
                "publish" if parts.len() >= 3 => {
                    let (bp, room) = parse_bp_room_key(&parts[1]).ok_or_else(|| anyhow::anyhow!("room must be <bp>:<room>"))?;
                    let event = parts[2].as_str();
                    let ok = registry.db.bp_script_publish(&bp, &room, event).await?;
                    if ok {
                        Ok(format!("[script] published {}:{} {}\n", bp, room, event))
                    } else {
                        Ok("[script] no draft found to publish.\n".into())
                    }
                }
                _ => Ok("Usage:\n  @script edit <bp>:<room> <event>\n  @script publish <bp>:<room> <event>\n".into()),
            }
        }
        _ => {
            // If in playtest, let Lua try to handle this as on_command
            let (bp, room, user) = {
                let s = sess.lock().await;
                match (&s.world, &s.name) {
                    (Some(WorldMode::Playtest { bp, room, .. }), Some(u)) => {
                        (bp.clone(), room.clone(), u.0.clone())
                    }
                    _ => return Ok("Unknown command. Try `help`.\n".into()),
                }
            };

            let args_vec = it
                .map(|s| s.to_string())
                .collect::<Vec<_>>();

            // Prepare the oneshot reply channel
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

            // Send job to the Lua worker
            lua_tx
                .send(crate::lua::LuaJob::OnCommandPlaytest {
                    db: registry.db.clone(),
                    bp,
                    room,
                    user,
                    verb: verb.to_string(),
                    args: args_vec,
                    reply: reply_tx,
                })
                .await
                .map_err(|_| anyhow::anyhow!("Lua worker dropped"))?;

            match reply_rx.await? {
                Ok(out) if !out.trim().is_empty() => Ok(out),
                _ => Ok("Unknown command. Try `help`.\n".into()),
            }
        }
    }
}

fn help_text() -> String {
    r#"
Available commands
------------------
  help                         Show this help
  register <name> <password>   Create a new account
  login <name> <password>      Log in (WebSocket or one-line)
  login <name>                 Log in (Telnet two-step; will prompt for password)
  who                          List online users
  look                         Look around your current room
  go <dir>                     Move (e.g., go north / go east)
  take coin [N]                Pick up up to N coins from the room
  balance                      Show how many coins you have
  quit                         Disconnect
"#
    .to_string()
}

fn parse_bp_room_key(s: &str) -> Option<(String, String)> {
    let (bp, room) = s.split_once(':')?;
    if bp.is_empty() || room.is_empty() {
        return None;
    }
    Some((bp.to_string(), room.to_string()))
}

fn split_args_quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_q = !in_q;
            }
            ' ' | '\t' if !in_q => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '\\' => {
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' {
                        cur.push(next);
                        chars.next();
                    } else {
                        cur.push(ch);
                    }
                } else {
                    cur.push(ch);
                }
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

async fn process_editor_line(
    line: &str,
    registry: &Arc<Registry>,
    sess: &Arc<Mutex<Session>>,
) -> anyhow::Result<String> {
    // finish with a single line: .end
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

    // otherwise accumulate
    {
        let mut s = sess.lock().await;
        if let Some(ed) = &mut s.editor {
            ed.buf.push_str(line);
            ed.buf.push('\n');
        }
    }
    Ok(String::new())
}
