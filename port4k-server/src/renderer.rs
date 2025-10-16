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
use once_cell::sync::Lazy;
use regex::Regex;

/// How many passes we allow for nested expansions (vars -> {o:..} -> colors -> ...)
const MAX_PASSES: usize = 3;

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

/// Pass order per iteration:
///   1) template vars/colors via `render_template_with_opts`
///   2) inline object tokens via `expand_inline_object_tokens`
/// Repeat until stable or MAX_PASSES reached.
fn render_template_multipass(template: &str, vars: &RenderVars, opts: &RenderOptions) -> String {
    let mut s = template.to_string();
    for _ in 0..MAX_PASSES {
        let before = s.clone();
        // Expand room/global vars + color tags known to the parser
        s = render_template_with_opts(&s, vars, opts);
        // Expand {o:id} placeholders *inside* any text produced above
        s = expand_inline_object_tokens(&s, vars);
        if s == before {
            break;
        }
    }
    s
}

/// Regex for {o:<id>} tokens. <id> allows [A-Za-z0-9_-]
static O_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{o:([A-Za-z0-9_\-]+)\}").unwrap());

/// Resolve object labels for {o:id}. We try a few common key shapes so you
/// don't have to change your `RenderVars` right away:
///   - "obj:<id>.short"
///   - "obj.<id>.short"
///   - "obj:<id>"           (already a label)
///   - "obj.<id>"           (already a label)
fn resolve_object_label(id: &str, vars: &RenderVars) -> Option<String> {
    let k1 = format!("obj:{}.short", id);
    let k2 = format!("obj.{}.short", id);
    let k3 = format!("obj:{}", id);
    let k4 = format!("obj.{}", id);

    vars.room_view
        .get(&k1).cloned()
        .or_else(|| vars.room_view.get(&k2).cloned())
        .or_else(|| vars.room_view.get(&k3).cloned())
        .or_else(|| vars.room_view.get(&k4).cloned())
}

/// Replace {o:id} with a nicely highlighted label.
/// If the object isn't known in `vars`, we leave the token intact (and you can
/// log upstream when building `RenderVars`).
fn expand_inline_object_tokens(s: &str, vars: &RenderVars) -> String {
    O_TAG_RE
        .replace_all(s, |caps: &regex::Captures| {
            let oid = &caps[1];
            match resolve_object_label(oid, vars) {
                Some(label) => format!("{{c:yellow:bold}}{}{{c}}", label),
                None => caps[0].to_string(), // keep as-is; avoids breaking authoring
            }
        })
        .into_owned()
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

    #[test]
    fn expands_object_token_basic() {
        let mut vars = RenderVars::default();
        // Common shape your vars::generate_render_vars can produce
        vars.room_view.insert("obj:toolkit.short".into(), "discarded toolkit".into());

        let tpl = "You notice a {o:toolkit} here.";
        // Use the multipass renderer so nested content (colors later) would resolve as well
        let out = super::render_template_multipass(tpl, &vars, &RenderOptions { missing_var: MissingVarPolicy::Color, max_width: 80 });

        assert!(!out.contains("{o:toolkit}"));
        assert!(out.contains("discarded toolkit"));
    }

    #[test]
    fn expands_object_token_with_dot_variant_key() {
        let mut vars = RenderVars::default();
        // Accepts dot variant too
        vars.room_view.insert("obj.toolkit.short".into(), "discarded toolkit".into());

        let tpl = "You notice a {o:toolkit} here.";
        let out = super::render_template_multipass(tpl, &vars, &RenderOptions { missing_var: MissingVarPolicy::Color, max_width: 80 });

        assert!(!out.contains("{o:toolkit}"));
        assert!(out.contains("discarded toolkit"));
    }

    #[test]
    fn multipass_resolves_colors_inside_object_label() {
        let mut vars = RenderVars::default();
        // The label itself contains color tags that should turn into ANSI on the next pass
        vars.room_view.insert("obj:map.short".into(), "{c:magenta}patched wall map{c}".into());

        let tpl = "On the bulkhead: {o:map}.";
        let out = super::render_template_multipass(tpl, &vars, &RenderOptions { missing_var: MissingVarPolicy::Color, max_width: 80 });

        // {o:map} is gone; label is present
        assert!(!out.contains("{o:map}"));
        assert!(out.contains("patched wall map"));
        // Color tags should have been converted to ANSI by a later pass
        assert!(out.contains("\x1b["));
    }

    #[test]
    fn leaves_unknown_object_token_intact() {
        let vars = RenderVars::default();
        let tpl = "There might be a {o:nonexistent} here.";
        let out = super::render_template_multipass(tpl, &vars, &RenderOptions { missing_var: MissingVarPolicy::Color, max_width: 80 });

        // Unknown tokens are preserved verbatim (authoring-safe)
        assert!(out.contains("{o:nonexistent}"));
    }

    #[test]
    fn recursion_is_bounded_by_max_passes() {
        let mut vars = RenderVars::default();
        // Pathological label that references itself. Each pass would try to expand again.
        // Our MAX_PASSES ensures we bail out and return whatever we have by then.
        vars.room_view.insert("obj:loop.short".into(), "see {o:loop}".into());

        let tpl = "Loop? {o:loop}";
        let out = super::render_template_multipass(tpl, &vars, &RenderOptions { missing_var: MissingVarPolicy::Color, max_width: 80 });

        // We must complete without hanging; after MAX_PASSES we'll still see the token.
        assert!(out.contains("{o:loop}"));
        // And we should see at least one expansion attempt around it (the yellow/bold wrapper)
        assert!(out.contains("\x1b["));
    }
}
