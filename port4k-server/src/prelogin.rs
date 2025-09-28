use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, warn};


use port4k_core::Username;
use crate::db::Db;

#[derive(Debug)]
pub struct Registry {
    pub db: Db,
    /// Online set.
    pub online: RwLock<std::collections::BTreeSet<String>>,
}

impl Registry {
    pub fn new(db: Db) -> Self {
        Self { db, online: RwLock::new(std::collections::BTreeSet::new()) }
    }

    pub async fn create_user(&self, name: &Username) -> anyhow::Result<bool> {
        self.db.create_user(name.0.as_str()).await
    }

    pub async fn user_exists(&self, name: &Username) -> bool {
        self.db.user_exists(name.0.as_str()).await.unwrap_or(false)
    }

    pub async fn set_online(&self, name: &Username, online: bool) {
        let mut g = self.online.write().await;
        if online { g.insert(name.0.clone()); } else { g.remove(&name.0); }
    }

    pub async fn who(&self) -> Vec<String> {
        self.online.read().await.iter().cloned().collect()
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState { PreLogin, LoggedIn }


#[derive(Debug)]
pub struct Session {
    pub name: Option<Username>,
    pub state: ConnState,
}


impl Default for Session { fn default() -> Self { Self { name: None, state: ConnState::PreLogin } } }

pub async fn handle_connection(stream: TcpStream, registry: Arc<Registry>, banner: &str, entry: &str) -> anyhow::Result<()> {
    let (r, mut w) = stream.into_split();
    let mut reader = BufReader::new(r);


    // Print banner and entry help
    w.write_all(banner.as_bytes()).await?;
    w.write_all(entry.as_bytes()).await?;
    w.write_all(b"> ").await?;
    w.flush().await?;


    let sess = Arc::new(Mutex::new(Session::default()));


    let mut line = String::new();
    while reader.read_line(&mut line).await? != 0 {
        let raw = line.trim_matches(['\r', '\n']).trim();
        debug!(%raw, "received line");
        let response = process_command(raw, &registry, &sess).await;
        line.clear();


        match response {
            Ok(mut out) => {
                if !out.ends_with('\n') { out.push('\n'); }
                w.write_all(out.as_bytes()).await?;
            }
            Err(e) => {
                warn!(error=%e, "command error");
                w.write_all(format!("error: {e}\n").as_bytes()).await?;
            }
        }


        w.write_all(b"> ").await?;
        w.flush().await?;
    }


    // On disconnect, mark offline if logged in
    let name = sess.lock().await.name.clone();
    if let Some(u) = name { registry.set_online(&u, false).await; }
    Ok(())
}

pub(crate) async fn process_command(cmd: &str, registry: &Arc<Registry>, sess: &Arc<Mutex<Session>>) -> anyhow::Result<String> {
    if cmd.is_empty() { return Ok(String::new()); }


    let mut it = cmd.split_whitespace();
    let Some(verb) = it.next() else { return Ok(String::new()) };


    match verb.to_ascii_lowercase().as_str() {
        "help" => Ok(help_text()),
        "quit" | "exit" => Ok("Goodbye!\n".to_string()),
        "who" => {
            let list = registry.who().await;
            if list.is_empty() { Ok("No one is online.\n".into()) } else { Ok(format!("Online ({}): {}\n", list.len(), list.join(", "))) }
        }
        "new" => {
            let name = it.collect::<Vec<_>>().join(" ");
            let Some(u) = Username::parse(&name) else { return Ok("Usage: new <alnum|_|->\n".into()) };
            if registry.create_user(&u).await? {
                Ok(format!("Created user `{}`. You can now `login {}`.\n", u, u))
            } else { Ok("That name is taken.\n".into()) }
        }
        "login" => {
            let name = it.collect::<Vec<_>>().join(" ");
            let Some(u) = Username::parse(&name) else { return Ok("Usage: login <name>\n".into()) };
            if !registry.user_exists(&u).await { return Ok("No such user. Try `new <name>`.\n".into()); }
            let mut s = sess.lock().await;
            s.name = Some(u.clone());
            s.state = ConnState::LoggedIn;
            registry.set_online(&u, true).await;
            Ok(format!("Welcome, {}! Type `look` or `help`.\n", u))
        }
        "look" => {
            let s = sess.lock().await;
            if s.state != ConnState::LoggedIn { return Ok("You must `login` first.\n".into()); }
            Ok("You are in a quiet void. Paths lead nowhere… yet.\n".into())
        }
        _ => Ok("Unknown command. Try `help`.\n".into()),
    }
}

fn help_text() -> String {
    r#"
Available commands
------------------
help          Show this help
new <name>    Create a new user (demo only)
login <name>  Log in as user (no password in demo)
who           List online users
look          Describe current location (demo)
quit          Disconnect
"#.to_string()
}