mod ansi;
mod parser;

pub mod room_view;
pub mod vars;

use crate::Session;
use crate::models::room::RoomView;
use crate::renderer::parser::Alignment;
use parking_lot::RwLock;
pub use parser::{Token, VarFmt};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct RenderVars {
    // Global values accessed with {v:var_name}
    pub global: HashMap<String, String>,
    // RoomView values accessed with {rv:var_name}
    pub room_view: HashMap<String, String>,
}

impl RenderVars {
    pub fn new(sess: Arc<RwLock<Session>>, rv: Option<&RoomView>) -> Self {
        vars::generate_render_vars(sess, rv)
    }
}

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
    pub max_width: usize,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            missing_var: MissingVarPolicy::LeaveToken,
            max_width: 80,
        }
    }
}

/// Public API: render a template with vars and default options.
pub fn render_template(template: &str, vars: &RenderVars, max_width: usize) -> String {
    render_template_with_opts(
        template,
        vars,
        &RenderOptions {
            missing_var: MissingVarPolicy::Color,
            max_width,
        },
    )
}

/// Public API: render with options.
pub fn render_template_with_opts(template: &str, vars: &RenderVars, opts: &RenderOptions) -> String {
    let tokens = parser::parse(template);
    let mut out = String::with_capacity(template.len() + 32);

    fn do_variable_substitution(
        chosen: Option<String>,
        raw: &str,
        fmt: &Option<VarFmt>,
        opts: &RenderOptions,
        out: &mut String,
    ) {
        match chosen {
            Some(val) => {
                let s = apply_format(fmt.as_ref(), &val);
                out.push_str(&s);
            }
            None => match opts.missing_var {
                MissingVarPolicy::Color => out.push_str(&format!("\x1b[36;41m{{{}}}\x1b[0m", raw)),
                MissingVarPolicy::LeaveToken => out.push_str(raw),
                MissingVarPolicy::Empty => {}
                MissingVarPolicy::Undefined => out.push_str("undefined"),
            },
        }
    }

    for t in tokens {
        match t {
            Token::Text(s) => out.push_str(&s),
            Token::RoomVar {
                raw,
                name,
                default,
                fmt,
            } => {
                let chosen = vars.room_view.get(&name).cloned().or(default);
                do_variable_substitution(chosen, &raw, &fmt, opts, &mut out);
            }
            Token::Var {
                raw,
                name,
                default,
                fmt,
            } => {
                let chosen = vars.global.get(&name).cloned().or(default);
                do_variable_substitution(chosen, &raw, &fmt, opts, &mut out);
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
        Some(VarFmt::String { width, alignment }) => {
            if let Some(w) = width {
                pad_string(value, *w as usize, alignment.clone())
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

fn pad_string(s: &str, width: usize, alignment: Alignment) -> String {
    if s.len() >= width {
        return s.to_string();
    }
    let pad = width - s.len();
    match alignment {
        Alignment::Center => {
            let left_pad = pad / 2;
            let right_pad = pad - left_pad;
            let mut out = String::with_capacity(width);
            for _ in 0..left_pad {
                out.push(' ');
            }
            out.push_str(s);
            for _ in 0..right_pad {
                out.push(' ');
            }
            out
        }
        Alignment::Left => {
            // left-align in a field -> pad on the right
            let mut out = String::with_capacity(width);
            out.push_str(s);
            for _ in 0..pad {
                out.push(' ');
            }
            out
        }
        Alignment::Right => {
            // right-align -> pad on the left
            let mut out = String::with_capacity(width);
            for _ in 0..pad {
                out.push(' ');
            }
            out.push_str(s);
            out
        }
    }
    // if left {
    //     // left-align in a field -> pad on the right
    //     let mut out = String::with_capacity(width);
    //     out.push_str(s);
    //     for _ in 0..pad { out.push(' '); }
    //     out
    // } else {
    //     // right-align -> pad on the left
    //     let mut out = String::with_capacity(width);
    //     for _ in 0..pad { out.push(' '); }
    //     out.push_str(s);
    //     out
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn var_default_and_format() {
        let mut vars = RenderVars {
            global: HashMap::new(),
            room_view: HashMap::new(),
        };
        vars.global.insert("score".into(), "7".into());
        let s = render_template("Score {v:score|%05d}", &vars, 80);
        assert_eq!(s, "Score 00007");
        let s = render_template("{v:missing:XYZ|%-6s}!", &vars, 80);
        assert_eq!(s, "XYZ   !");
    }

    #[test]
    fn colors() {
        let vars = RenderVars::default();
        let s = render_template("{c:yellow:bold}warn{c}", &vars, 80);
        assert!(s.contains("\x1b[33m") || s.contains("\x1b[33;1m")); // depending on join order
        assert!(s.ends_with("\x1b[0m"));
    }

    #[test]
    fn escapes() {
        let vars = RenderVars::default();
        let s = render_template("{{v}} -> {v:name}", &vars, 80);
        assert_eq!(s, "{v} -> \u{1b}[36;41m{{v:name}}\u{1b}[0m");
    }

    #[test]
    fn string_padding() {
        let mut vars = RenderVars::default();
        vars.global.insert("name".into(), "Ada".into());
        let s = render_template("Hello {v:name|%-6s}!", &vars, 80);
        assert_eq!(s, "Hello Ada   !");
        let s = render_template("Hello {v:name|%6s}!", &vars, 80);
        assert_eq!(s, "Hello    Ada!");
    }
}

// use once_cell::sync::Lazy;
// use regex::Regex;
// use std::sync::Arc;
// use parking_lot::RwLock;
// use crate::models::room::{RoomExitRow, RoomObject, RoomView};
// use crate::Session;
//
// pub struct Theme {
//     pub room_title: String,
//     pub room_body: String,
//     pub objects: String,
//     pub exits: String,
//     pub exit_preface: String,
// }
//
// impl Theme {
//     pub fn blue() -> Self {
//         Self {
//             room_title: "\x1b[38;5;75;1m".to_string(), // bright sky blue
//             room_body: "\x1b[0m".to_string(),          // normal
//             objects: "\x1b[38;5;81m".to_string(),      // cyan/teal
//             exits: "\x1b[38;5;39;1m".to_string(),      // vivid blue
//             exit_preface: "\x1b[38;5;39m".to_string(), // normal blue
//         }
//     }
//
//     /// 16-color safe fallback
//     #[allow(unused)]
//     pub fn ansi16() -> Self {
//         Self {
//             room_title: "\x1b[1;36m".to_string(), // bright cyan
//             room_body: "\x1b[0m".to_string(),
//             objects: "\x1b[36m".to_string(),      // cyan
//             exits: "\x1b[1;34m".to_string(),      // bright blue
//             exit_preface: "\x1b[34m".to_string(), // normal blue
//         }
//     }
// }

// const RESET: &str = "\x1b[0m";
//
// static ANSI_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*m").unwrap());
// static OBJ_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{obj:([a-zA-Z0-9_\-:]+)(?:\|([^}]+))?}").unwrap());

// fn render_objects(theme: &Theme, body: &str, objects: Vec<RoomObject>) -> String {
//     let col1 = &theme.room_body;
//     let col2 = &theme.objects;
//
//     let id_to_short: HashMap<String, String> = objects
//         .into_iter()
//         .map(|o| (o.name, o.short))
//         .collect();
//
//     let s = OBJ_TAG_RE
//         .replace_all(body, |caps: &regex::Captures| {
//             let id = &caps[1];
//             // label override if provided: {obj:id|label}
//             let label = caps
//                 .get(2)
//                 .map(|m| m.as_str().to_string())
//                 .or_else(|| id_to_short.get(id).cloned())
//                 .unwrap_or_else(|| id.to_string()); // fallback to id if unknown (author typo etc.)
//
//             format!("{RESET}{col2}{label}{RESET}{col1}")
//         })
//         .into_owned();
//
//     format!("{col1}{s}{RESET}")
// }

// Compute visible length ignoring ANSI escape codes
// fn visible_len(s: &str) -> usize {
//     ANSI_RE.replace_all(s, "").chars().count()
// }

// /// ANSI-aware word wrap (simple greedy wrap). Preserves paragraphs and explicit newlines.
// fn wrap_ansi(text: &str, width: usize) -> String {
//     // Split on blank lines to preserve paragraphs
//     let mut out = String::new();
//     for (pi, para) in text.split("\n\n").enumerate() {
//         if pi > 0 {
//             out.push_str("\n\n");
//         }
//
//         // For each paragraph, wrap each line but reflow spaces
//         let mut line_len = 0usize;
//         let mut first_word = true;
//
//         // Treat any whitespace as separators; keep words intact
//         for word in para.split_whitespace() {
//             let w_len = visible_len(word);
//             let sep = if first_word { "" } else { " " };
//             let add = if first_word { w_len } else { 1 + w_len };
//
//             if line_len > 0 && line_len + 1 + w_len > width {
//                 out.push('\n');
//                 out.push_str(word);
//                 line_len = w_len;
//                 first_word = false;
//             } else {
//                 out.push_str(sep);
//                 out.push_str(word);
//                 line_len += add;
//                 first_word = false;
//             }
//         }
//     }
//     out
// }

// fn color_title(theme: &Theme, title: &str) -> String {
//     let col = &theme.room_title;
//     format!("{col}{title}{RESET}")
// }
//
// fn color_exits(theme: &Theme, exits: &[RoomExitRow]) -> String {
//     let col1 = &theme.exit_preface;
//     let col2 = &theme.exits;
//
//     if exits.is_empty() {
//         format!("{col1}Exits:{RESET} none")
//     } else {
//         let exits: Vec<String> = exits.iter().map(|e| e.dir.to_string()).collect();
//         let joined = exits.join(", ");
//         format!("{col1}Exits:{RESET} {col2}{joined}{RESET}")
//     }
// }

// pub fn render_room(
//     theme: &Theme,
//     width: usize,
//     room: RoomView,
// ) -> String {
//     let border = color_title(theme, &"-".repeat(room.room.title.len().min(80)));
//     let title_line = color_title(theme, room.room.title.as_str());
//     let body_highlighted = render_objects(theme, room.room.body.as_str(), room.objects);
//     let body_wrapped = wrap_ansi(body_highlighted.as_str(), width.max(20));
//     let exits_line = color_exits(theme, room.exits.as_slice());
//
//     format!("{border}\n{title_line}\n{border}\n\n{body_wrapped}\n\n{exits_line}\n")
// }

// pub fn render_text(sess: Arc<RwLock<Session>>, _theme: &Theme, _width: usize, text: &str) -> String {
//     let vars = get_vars(sess.clone());
//     render(text, &vars)
// }
