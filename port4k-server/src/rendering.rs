use std::collections::HashMap;
use regex::Regex;
use once_cell::sync::Lazy;

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
            room_title: "\x1b[38;5;75;1m".to_string(),   // bright sky blue
            room_body: "\x1b[0m".to_string(),            // normal
            objects: "\x1b[38;5;81m".to_string(),        // cyan/teal
            exits: "\x1b[38;5;39;1m".to_string(),        // vivid blue
            exit_preface: "\x1b[38;5;39m".to_string(),   // normal blue
        }
    }

    /// 16-color safe fallback
    #[allow(unused)]
    pub fn ansi16() -> Self {
        Self {
            room_title: "\x1b[1;36m".to_string(),   // bright cyan
            room_body: "\x1b[0m".to_string(),
            objects: "\x1b[36m".to_string(),        // cyan
            exits: "\x1b[1;34m".to_string(),        // bright blue
            exit_preface: "\x1b[34m".to_string(),   // normal blue
        }
    }
}
//
// /// Basic ANSI bits (portable across most telnet clients)
const RESET: &str = "\x1b[0m";
// const BOLD: &str = "\x1b[1m";
// const UNDERLINE: &str = "\x1b[4m";
// const FG_CYAN: &str = "\x1b[36m";
// // const FG_YELLOW: &str = "\x1b[33m";
// const FG_BLUE: &str = "\x1b[34m";
// const FG_BLUE_BRIGHT: &str = "\x1b[94m";
// const FG_WHITE_BRIGHT: &str = "\x1b[97m";
//
// const FG_TITLE: &str = "\x1b[1;36m";
// const FG_BORDER_TITLE: &str = "\x1b[1;35m";
// const FG_OBJECTS: &str = "\x1b[4m\x1b[1;36m";
// const FG_EXITS: &str = "\x1b[1;34m";

static ANSI_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*m").unwrap());
static OBJ_TAG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{obj:([a-zA-Z0-9_\-:]+)(?:\|([^}]+))?\}").unwrap());

fn render_objects(theme: &Theme, body: &str, id_to_short: &HashMap<String, String>) -> String {
    let col1 = &theme.room_body;
    let col2 = &theme.objects;

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
    for (pi, para) in text.split("\r\n\r\n").enumerate() {
        if pi > 0 { out.push_str("\r\n\r\n"); }

        // For each paragraph, wrap each line but reflow spaces
        let mut line_len = 0usize;
        let mut first_word = true;

        // Treat any whitespace as separators; keep words intact
        for word in para.split_whitespace() {
            let wlen = visible_len(word);
            let sep = if first_word { "" } else { " " };
            let add = if first_word { wlen } else { 1 + wlen };

            if line_len > 0 && line_len + 1 + wlen > width {
                out.push('\r');
                out.push('\n');
                out.push_str(word);
                line_len = wlen;
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

fn color_title(theme: &Theme,title: &str) -> String {
    let col = &theme.room_title;
    format!("{col}{title}{RESET}")
}

fn color_exits(theme: &Theme,dirs: &[String]) -> String {
    let col1 = &theme.exit_preface;
    let col2 = &theme.exits;

    if dirs.is_empty() {
        format!("{col1}Exits:{RESET} none")
    } else {
        let joined = dirs.join(", ");
        format!("{col1}Exits:{RESET} {col2}{joined}{RESET}")
    }
}

pub fn render_room(
    theme: &Theme,
    title: &str,
    body: &str,
    objects: &HashMap<String, String>,
    exits: &[String],
    width: usize,
) -> String {
    let border = color_title(theme, &"-".repeat(title.len().min(80)));
    let title_line = color_title(theme, title);
    let body_highlighted = render_objects(theme, body, objects);
    let body_wrapped = wrap_ansi(body_highlighted.as_str(), width.max(20));
    let exits_line = color_exits(theme, exits);

    format!("{border}\r\n{title_line}\r\n{border}\r\n\r\n{body_wrapped}\r\n\r\n{exits_line}\r\n")
}
