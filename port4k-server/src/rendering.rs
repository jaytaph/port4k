use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

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
static OBJ_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{obj:([a-zA-Z0-9_\-:]+)(?:\|([^}]+))?\}").unwrap());

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
    for (pi, para) in text.split("\n\n").enumerate() {
        if pi > 0 {
            out.push_str("\n\n");
        }

        // For each paragraph, wrap each line but reflow spaces
        let mut line_len = 0usize;
        let mut first_word = true;

        // Treat any whitespace as separators; keep words intact
        for word in para.split_whitespace() {
            let wlen = visible_len(word);
            let sep = if first_word { "" } else { " " };
            let add = if first_word { wlen } else { 1 + wlen };

            if line_len > 0 && line_len + 1 + wlen > width {
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

fn color_title(theme: &Theme, title: &str) -> String {
    let col = &theme.room_title;
    format!("{col}{title}{RESET}")
}

fn color_exits(theme: &Theme, dirs: &[String]) -> String {
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

    format!("{border}\n{title_line}\n{border}\n\n{body_wrapped}\n\n{exits_line}\n")
}
