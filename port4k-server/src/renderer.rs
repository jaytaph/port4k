mod parser;
mod ansi;

use std::collections::HashMap;

pub use parser::{Token, VarFmt};

/// What to do when a variable is missing and has no default.
#[derive(Clone, Copy, Debug)]
pub enum MissingVarPolicy {
    /// Render in red on cyan background (ANSI)
    Color,
    /// Leave the original token text: `{v:name}`
    LeaveToken,
    /// Render as empty string
    Empty,
    /// Use the word "undefined"
    Undefined,
}

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub missing_var: MissingVarPolicy,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { missing_var: MissingVarPolicy::LeaveToken }
    }
}

/// Public API: render a template with vars and default options.
pub fn render(template: &str, vars: &HashMap<String, String>) -> String {
    render_with_opts(template, vars, &RenderOptions{
        missing_var: MissingVarPolicy::Color,
    })
}

/// Public API: render with options.
pub fn render_with_opts(template: &str, vars: &HashMap<String, String>, opts: &RenderOptions) -> String {
    let tokens = parser::parse(template);
    let mut out = String::with_capacity(template.len() + 32);

    for t in tokens {
        match t {
            Token::Text(s) => out.push_str(&s),
            Token::Var { raw, name, default, fmt } => {
                let chosen = vars.get(&name).cloned().or(default);
                match chosen {
                    Some(val) => {
                        let s = apply_format(fmt.as_ref(), &val);
                        out.push_str(&s);
                    }
                    None => {
                        match opts.missing_var {
                            MissingVarPolicy::Color => out.push_str(&format!("\x1b[36;41m{{{}}}\x1b[0m", raw)),
                            MissingVarPolicy::LeaveToken => out.push_str(&raw),
                            MissingVarPolicy::Empty => {}
                            MissingVarPolicy::Undefined => out.push_str("undefined"),
                        }
                    }
                }
            }
            Token::ColorReset => out.push_str(ansi::RESET),
            Token::Color { fg, bg, attrs } => {
                let code = ansi::compose_sgr(fg.as_deref(), bg.as_deref(), &attrs);
                if !code.is_empty() {
                    out.push_str(&code);
                }
            }
            Token::Unknown(raw) => out.push_str(&raw), // pass-through for forward-compat
        }
    }

    out
}

/// Minimal formatter that supports:
/// %s, %-Ns, %Ns  (pad with spaces)
/// %d, %0Nd, %Nd  (integer, zero/space pad)
fn apply_format(fmt: Option<&VarFmt>, value: &str) -> String {
    match fmt {
        None => value.to_string(),
        Some(VarFmt::String { width, left }) => {
            if let Some(w) = width {
                pad_string(value, *w as usize, *left)
            } else {
                value.to_string()
            }
        }
        Some(VarFmt::Int { width, zero_pad }) => {
            let n = value.parse::<i64>().unwrap_or(0);
            if let Some(w) = width {
                if *zero_pad {
                    format!("{:0width$}", n, width = *w as usize)
                } else {
                    format!("{:>width$}", n, width = *w as usize)
                }
            } else {
                n.to_string()
            }
        }
    }
}

fn pad_string(s: &str, width: usize, left: bool) -> String {
    if s.len() >= width { return s.to_string(); }
    let pad = width - s.len();
    if left {
        // left-align in a field -> pad on the right
        let mut out = String::with_capacity(width);
        out.push_str(s);
        for _ in 0..pad { out.push(' '); }
        out
    } else {
        // right-align -> pad on the left
        let mut out = String::with_capacity(width);
        for _ in 0..pad { out.push(' '); }
        out.push_str(s);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn var_default_and_format() {
        let mut vars = HashMap::new();
        vars.insert("score".into(), "7".into());
        let s = render("Score {v:score|%05d}", &vars);
        assert_eq!(s, "Score 00007");
        let s = render("{v:missing:XYZ|%-6s}!", &vars);
        assert_eq!(s, "XYZ   !");
    }

    #[test]
    fn colors() {
        let vars = HashMap::new();
        let s = render("{c:yellow:bold}warn{c}", &vars);
        assert!(s.contains("\x1b[33m") || s.contains("\x1b[33;1m")); // depending on join order
        assert!(s.ends_with("\x1b[0m"));
    }

    #[test]
    fn escapes() {
        let vars = HashMap::new();
        let s = render("{{v}} -> {v:name}", &vars);
        assert_eq!(s, "{v} -> {v:name}");
    }

    #[test]
    fn string_padding() {
        let mut vars = HashMap::new();
        vars.insert("name".into(), "Ada".into());
        let s = render("Hello {v:name|%-6s}!", &vars);
        assert_eq!(s, "Hello Ada   !");
        let s = render("Hello {v:name|%6s}!", &vars);
        assert_eq!(s, "Hello    Ada!");
    }
}



use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::models::room::{RoomExitRow, RoomObject, RoomView};
use crate::Session;

pub struct Theme {
    pub room_title: String,
    pub room_body: String,
    pub objects: String,
    pub exits: String,
    pub exit_preface: String,
}

impl Theme {
    pub fn blue() -> Self {
        Self {
            room_title: "\x1b[38;5;75;1m".to_string(), // bright sky blue
            room_body: "\x1b[0m".to_string(),          // normal
            objects: "\x1b[38;5;81m".to_string(),      // cyan/teal
            exits: "\x1b[38;5;39;1m".to_string(),      // vivid blue
            exit_preface: "\x1b[38;5;39m".to_string(), // normal blue
        }
    }

    /// 16-color safe fallback
    #[allow(unused)]
    pub fn ansi16() -> Self {
        Self {
            room_title: "\x1b[1;36m".to_string(), // bright cyan
            room_body: "\x1b[0m".to_string(),
            objects: "\x1b[36m".to_string(),      // cyan
            exits: "\x1b[1;34m".to_string(),      // bright blue
            exit_preface: "\x1b[34m".to_string(), // normal blue
        }
    }
}

const RESET: &str = "\x1b[0m";

static ANSI_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*m").unwrap());
static OBJ_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{obj:([a-zA-Z0-9_\-:]+)(?:\|([^}]+))?}").unwrap());

fn render_objects(theme: &Theme, body: &str, objects: Vec<RoomObject>) -> String {
    let col1 = &theme.room_body;
    let col2 = &theme.objects;

    let id_to_short: HashMap<String, String> = objects
        .into_iter()
        .map(|o| (o.name, o.short))
        .collect();

    let s = OBJ_TAG_RE
        .replace_all(body, |caps: &regex::Captures| {
            let id = &caps[1];
            // label override if provided: {obj:id|label}
            let label = caps
                .get(2)
                .map(|m| m.as_str().to_string())
                .or_else(|| id_to_short.get(id).cloned())
                .unwrap_or_else(|| id.to_string()); // fallback to id if unknown (author typo etc.)

            format!("{RESET}{col2}{label}{RESET}{col1}")
        })
        .into_owned();

    format!("{col1}{s}{RESET}")
}

/// Compute visible length ignoring ANSI escape codes
fn visible_len(s: &str) -> usize {
    ANSI_RE.replace_all(s, "").chars().count()
}

/// ANSI-aware word wrap (simple greedy wrap). Preserves paragraphs and explicit newlines.
fn wrap_ansi(text: &str, width: usize) -> String {
    // Split on blank lines to preserve paragraphs
    let mut out = String::new();
    for (pi, para) in text.split("\n\n").enumerate() {
        if pi > 0 {
            out.push_str("\n\n");
        }

        // For each paragraph, wrap each line but reflow spaces
        let mut line_len = 0usize;
        let mut first_word = true;

        // Treat any whitespace as separators; keep words intact
        for word in para.split_whitespace() {
            let w_len = visible_len(word);
            let sep = if first_word { "" } else { " " };
            let add = if first_word { w_len } else { 1 + w_len };

            if line_len > 0 && line_len + 1 + w_len > width {
                out.push('\n');
                out.push_str(word);
                line_len = w_len;
                first_word = false;
            } else {
                out.push_str(sep);
                out.push_str(word);
                line_len += add;
                first_word = false;
            }
        }
    }
    out
}

fn color_title(theme: &Theme, title: &str) -> String {
    let col = &theme.room_title;
    format!("{col}{title}{RESET}")
}

fn color_exits(theme: &Theme, exits: &[RoomExitRow]) -> String {
    let col1 = &theme.exit_preface;
    let col2 = &theme.exits;

    if exits.is_empty() {
        format!("{col1}Exits:{RESET} none")
    } else {
        let exits: Vec<String> = exits.iter().map(|e| e.dir.to_string()).collect();
        let joined = exits.join(", ");
        format!("{col1}Exits:{RESET} {col2}{joined}{RESET}")
    }
}

pub fn render_room(
    theme: &Theme,
    width: usize,
    room: RoomView,
) -> String {
    let border = color_title(theme, &"-".repeat(room.room.title.len().min(80)));
    let title_line = color_title(theme, room.room.title.as_str());
    let body_highlighted = render_objects(theme, room.room.body.as_str(), room.objects);
    let body_wrapped = wrap_ansi(body_highlighted.as_str(), width.max(20));
    let exits_line = color_exits(theme, room.exits.as_slice());

    format!("{border}\n{title_line}\n{border}\n\n{body_wrapped}\n\n{exits_line}\n")
}

pub fn get_vars(sess: Arc<RwLock<Session>>) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Generic vars not tied to account or location
    vars.insert("wall_time".to_string(), chrono::Local::now().format("%H:%M:%S").to_string());
    vars.insert("online_time".to_string(), format!("{}", sess.read().session_started.elapsed().as_secs()));
    vars.insert("online_users".to_string(), format!("{}", 123));
    vars.insert("unread_messages".to_string(), format!("{}", 0));
    vars.insert("active_quests".to_string(), format!("{}", 0));
    vars.insert("now_utc".to_string(), chrono::Utc::now().to_rfc3339());
    vars.insert("now_local".to_string(), chrono::Local::now().to_rfc3339());

    if let Some(account) = sess.read().account.as_ref() {
        vars.insert("account.name".to_string(), account.username.to_string());
        vars.insert("account.role".to_string(), account.role.to_string());
        vars.insert("account.xp".to_string(), format!("{}", account.xp));
        vars.insert("account.health".to_string(), format!("{}", account.health));
        vars.insert("account.coins".to_string(), format!("{}", account.coins));
    }
    if let Some(cursor) = sess.read().cursor.as_ref() {
        vars.insert("cursor.zone".to_string(), cursor.zone_ctx.zone.title.to_string());
        vars.insert("cursor.room.title".to_string(), cursor.room_view.room.title.to_string());
        // vars.insert("cursor.view".to_string(), cursor.room.title.to_string());
    }

    vars
}

pub fn resolve_vars(
    template: &str,
    vars: HashMap<String, String>,
) -> String {
    let re = Regex::new(r"\{([a-zA-Z0-9_]+)}").unwrap();
    re.replace_all(template, |caps: &regex::Captures| {
        vars.get(&caps[1]).cloned().unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}


pub fn render_text(sess: Arc<RwLock<Session>>, _theme: &Theme, _width: usize, text: &str) -> String {
    let vars = get_vars(sess.clone());
    render(text, &vars)

    // let text = resolve_vars(text, vars);
    //
    // let body_wrapped = wrap_ansi(&text, width.max(20));
    // format!("{}{}\n", theme.room_body, body_wrapped)
}