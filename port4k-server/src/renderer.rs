mod ansi;
mod parser;

mod objects;
pub mod room_view;
pub mod vars;

use crate::Session;
use crate::renderer::parser::Alignment;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
pub use parser::{Token, VarFmt};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

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
    pub fn with(mut self, key: &str, val: &str) -> Self {
        self.global.insert(key.to_string(), val.to_string());
        self
    }

    pub fn without(mut self, key: &str) -> Self {
        self.global.remove(key);
        self
    }
}

impl std::fmt::Debug for RenderVars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn sorted_map_display<T: std::fmt::Debug>(
            f: &mut std::fmt::Formatter<'_>,
            name: &str,
            map: &HashMap<String, T>,
        ) -> std::fmt::Result {
            let mut items: Vec<_> = map.iter().collect();
            items.sort_by_key(|(k, _)| *k);

            // Print like: name: { key1 = val1, key2 = val2, ... }
            writeln!(f, "{}:", name)?;
            for (k, v) in items {
                writeln!(f, "  {:<25} = {:?}", k, v)?;
            }
            Ok(())
        }

        writeln!(f, "RenderVars {{")?;
        sorted_map_display(f, "global", &self.global)?;
        sorted_map_display(f, "room_view", &self.room_view)?;
        writeln!(f, "}}")
    }
}

impl RenderVars {
    pub fn new(sess: Arc<RwLock<Session>>) -> Self {
        vars::generate_render_vars(sess)
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
    /// What to do with missing vars
    pub missing_var: MissingVarPolicy,
    /// Maximum width for the rendered output
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

fn render_single_pass(template: &str, vars: &RenderVars, opts: &RenderOptions) -> String {
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
            Token::Var(v) => {
                use parser::Scope::*;
                match v.scope {
                    Global => {
                        let chosen = vars.global.get(&v.key).cloned().or(v.default.clone());
                        do_variable_substitution(chosen, &v.raw, &v.fmt, opts, &mut out);
                    }
                    Room => {
                        let chosen = vars.room_view.get(&v.key).cloned().or(v.default.clone());
                        do_variable_substitution(chosen, &v.raw, &v.fmt, opts, &mut out);
                    }
                    Object => {
                        let tmp = expand_inline_object_tokens(&v.raw, vars);
                        out.push_str(&tmp);
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
            Token::Debug(d) => out.push_str(&d.raw), // pass-through (design mode handled elsewhere)
            Token::Unknown(raw) => out.push_str(&raw),
        }
    }

    out
}

/// Regex for {o:<id>} tokens. <id> allows [A-Za-z0-9_-]
static O_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{(?:o|obj):([A-Za-z0-9_\-]+)\}").unwrap());

/// Resolve object labels for {o:id}. We try a few common key shapes so you
/// don't have to change your `RenderVars` right away:
///   - "obj.<id>.short"
///   - "obj.<id>"           (already a label)
fn resolve_object_label(id: &str, vars: &RenderVars) -> Option<String> {
    let k1 = format!("obj.{}.short", id);
    let k2 = format!("obj.{}", id);

    vars.room_view
        .get(&k1)
        .cloned()
        .or_else(|| vars.room_view.get(&k2).cloned())
}

/// Replace {o:id} with a nicely highlighted label.
/// If the object isn't known in `vars`, we leave the token intact (and you can
/// log upstream when building `RenderVars`).
fn expand_inline_object_tokens(s: &str, vars: &RenderVars) -> String {
    O_TAG_RE
        .replace_all(s, |caps: &regex::Captures| {
            let oid = &caps[1];
            match resolve_object_label(oid, vars) {
                Some(label) => format!("{{c:bright_yellow}}{}{{c}}", label),
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
    let mut s = template.to_string();
    for _ in 0..MAX_PASSES {
        let before = s.clone();
        // pass 1: vars + colors (parser-driven)
        s = render_single_pass(&s, vars, opts);
        // pass 2: {o:...} (regex-driven)
        s = expand_inline_object_tokens(&s, vars);
        if s == before {
            break;
        }
    }

    if opts.max_width > 0 {
        wrap_ansi_aware(&s, opts.max_width)
    } else {
        s
    }
}

/// Minimal formatter that supports:
/// %s, %-Ns, %*Ns, %Ns  (pad with spaces; %* is center)
/// %d, %0Nd, %Nd        (integer, zero/space pad)
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
            let trimmed = value.trim();
            let n = trimmed.parse::<i64>().ok().unwrap_or(0);
            if let Some(w) = width {
                if *zero_pad {
                    format!("{n:0width$}", n = n, width = *w as usize)
                } else {
                    format!("{n:>width$}", n = n, width = *w as usize)
                }
            } else {
                n.to_string()
            }
        }
    }
}

fn pad_string(s: &str, width: usize, alignment: Alignment) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.to_string();
    }
    let pad = width - len;
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
            let mut out = String::with_capacity(width);
            out.push_str(s);
            for _ in 0..pad {
                out.push(' ');
            }
            out
        }
        Alignment::Right => {
            let mut out = String::with_capacity(width);
            for _ in 0..pad {
                out.push(' ');
            }
            out.push_str(s);
            out
        }
    }
}

/// =======================
/// ANSI-aware soft wrapper
/// =======================

static ANSI_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1B\[[0-9;]*m").unwrap());
static WS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

fn visible_len(s: &str) -> usize {
    ANSI_RE.replace_all(s, "").chars().count()
}

fn wrap_ansi_aware(input: &str, width: usize) -> String {
    if width == 0 {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len() + input.len() / width);

    for (i, raw_line) in input.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }

        let mut line = String::new();
        let mut line_vis = 0usize;
        let mut pending_ws = String::new();

        // Walk the line while preserving BOTH words and whitespace runs
        let mut last = 0usize;
        for m in WS_RE.find_iter(raw_line) {
            let (start, end) = (m.start(), m.end());

            // preceding "word" (could contain ANSI)
            if start > last {
                let word = &raw_line[last..start];
                process_token(word, &mut pending_ws, &mut line, &mut line_vis, width, &mut out);
            }

            // the whitespace run (exactly as authored)
            let ws = &raw_line[start..end];

            if line.is_empty() && last == 0 && start == 0 {
                // Preserve leading whitespace immediately
                line.push_str(ws);
                // Count toward visible width (treat each byte as one column; adjust if you handle tabs specially)
                line_vis += ws.len();
                pending_ws.clear();
            } else {
                // Defer inter-word whitespace until the next token
                pending_ws.clear();
                pending_ws.push_str(ws);
            }

            last = end;
        }

        // trailing word after the last whitespace match
        if last < raw_line.len() {
            let word = &raw_line[last..];
            process_token(word, &mut pending_ws, &mut line, &mut line_vis, width, &mut out);
        }

        // flush trailing pending whitespace too — we preserve leading/trailing spaces
        line.push_str(&pending_ws);
        pending_ws.clear();

        out.push_str(&line);
    }

    out
}

// Helper: place pending whitespace + next word if it fits; otherwise wrap first.
#[inline]
fn process_token(
    word: &str,
    pending_ws: &mut String,
    line: &mut String,
    line_vis: &mut usize,
    width: usize,
    out: &mut String,
) {
    let ws_vis = visible_len(&pending_ws);
    let tok_vis = visible_len(word);

    if *line_vis == 0 {
        line.push_str(&pending_ws); // preserve leading spaces
        line.push_str(word);
        *line_vis += tok_vis;
        pending_ws.clear();
        return;
    }

    if *line_vis + ws_vis + tok_vis <= width {
        // it fits: emit exact spaces then the word
        line.push_str(&pending_ws);
        line.push_str(word);
        *line_vis += ws_vis + tok_vis;
        pending_ws.clear();
    } else {
        // wrap BEFORE spaces and word
        out.push_str(line);
        out.push('\n');
        line.clear();
        line.push_str(word);
        *line_vis = tok_vis;
        pending_ws.clear(); // drop leading spaces on new line
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
        let s = render_template("{c:bright_yellow}warn{c}", &vars, 80);
        assert!(s.contains("\x1b[93"));
        assert!(s.ends_with("\x1b[0m"));
    }

    #[test]
    fn escapes() {
        let vars = RenderVars::default();
        let opts = RenderOptions {
            missing_var: MissingVarPolicy::LeaveToken,
            max_width: 80,
        };
        let s = render_template_with_opts("{{v}} -> {v:name}", &vars, &opts);
        assert_eq!(s, "{v} -> {v:name}");
    }

    #[test]
    fn string_padding() {
        let mut vars = RenderVars::default();
        vars.global.insert("name".into(), "Ada".into());
        let s = render_template("Hell1 {v:name|%-6s}!", &vars, 80);
        assert_eq!(s, "Hell1 Ada   !");
        let s = render_template("Hell2 {v:name|%6s}!", &vars, 80);
        assert_eq!(s, "Hell2    Ada!");

        let s = render_template("Hell3 {v:name|%-9s}!", &vars, 80);
        assert_eq!(s, "Hell3 Ada      !");
        let s = render_template("Hell4 {v:name|%9s}!", &vars, 80);
        assert_eq!(s, "Hell4       Ada!");
        let s = render_template("Hell5 {v:name|%*9s}!", &vars, 80);
        assert_eq!(s, "Hell5    Ada   !");
    }

    #[test]
    fn expands_object_token_basic() {
        let mut vars = RenderVars::default();
        vars.room_view
            .insert("obj.toolkit.short".into(), "discarded toolkit".into());

        let tpl = "You notice a {o:toolkit} here.";
        let out = super::render_template_with_opts(
            tpl,
            &vars,
            &RenderOptions {
                missing_var: MissingVarPolicy::Color,
                max_width: 80,
            },
        );

        assert!(!out.contains("{o:toolkit}"));
        assert!(out.contains("discarded toolkit"));
    }

    #[test]
    fn expands_object_token_with_dot_variant_key() {
        let mut vars = RenderVars::default();
        vars.room_view
            .insert("obj.toolkit.short".into(), "discarded toolkit".into());

        let tpl = "You notice a {o:toolkit} here.";
        let out = super::render_template_with_opts(
            tpl,
            &vars,
            &RenderOptions {
                missing_var: MissingVarPolicy::Color,
                max_width: 80,
            },
        );

        assert!(!out.contains("{o:toolkit}"));
        assert!(out.contains("discarded toolkit"));
    }

    #[test]
    fn multipass_resolves_colors_inside_object_label() {
        let mut vars = RenderVars::default();
        vars.room_view
            .insert("obj.map.short".into(), "{c:magenta}patched wall map{c}".into());

        let tpl = "On the bulkhead: {o:map}.";
        let out = super::render_template_with_opts(
            tpl,
            &vars,
            &RenderOptions {
                missing_var: MissingVarPolicy::Color,
                max_width: 80,
            },
        );

        assert!(!out.contains("{o:map}"));
        assert!(out.contains("patched wall map"));
        assert!(out.contains("\x1b["));
    }

    #[test]
    fn leaves_unknown_object_token_intact() {
        let vars = RenderVars::default();
        let tpl = "There might be a {o:nonexistent} here.";
        let out = super::render_template_with_opts(
            tpl,
            &vars,
            &RenderOptions {
                missing_var: MissingVarPolicy::Color,
                max_width: 80,
            },
        );

        assert!(out.contains("{o:nonexistent}"));
    }

    #[test]
    fn recursion_is_bounded_by_max_passes() {
        let mut vars = RenderVars::default();
        vars.room_view.insert("obj.loop.short".into(), "see {o:loop}".into());

        let tpl = "Loop? {o:loop}";
        let out = super::render_template_with_opts(
            tpl,
            &vars,
            &RenderOptions {
                missing_var: MissingVarPolicy::Color,
                max_width: 80,
            },
        );

        assert!(out.contains("{o:loop}"));
        assert!(out.contains("\x1b["));
    }

    #[test]
    fn soft_wrap_respects_ansi_and_words() {
        let mut vars = RenderVars::default();
        vars.global
            .insert("msg".into(), "Alpha Beta Gamma Delta Epsilon Zeta".into());

        // Add some ANSI around a word; should not count toward width
        let tpl = "{c:bright_yellow}{v:msg}{c}";
        let s = render_template_with_opts(
            tpl,
            &vars,
            &RenderOptions {
                missing_var: MissingVarPolicy::Color,
                max_width: 20,
            },
        );

        // Expect at least one newline due to wrapping near 20 visible chars
        assert!(s.contains('\n'));

        // All words preserved (no splitting mid-word)
        let stripped = ANSI_RE.replace_all(&s, "");
        assert!(stripped.contains("Alpha"));
        assert!(stripped.contains("Beta"));
        assert!(stripped.contains("Gamma"));
        assert!(stripped.contains("Delta"));
        assert!(stripped.contains("Epsilon"));
        assert!(stripped.contains("Zeta"));
    }

    #[test]
    fn pad_string_left_alignment() {
        // width > len -> pad on the right
        assert_eq!(pad_string("Ada", 6, Alignment::Left), "Ada   ");
        // width <= len -> unchanged
        assert_eq!(pad_string("Ada", 3, Alignment::Left), "Ada");
        assert_eq!(pad_string("Ada", 2, Alignment::Left), "Ada");
    }

    #[test]
    fn pad_string_right_alignment() {
        // width > len -> pad on the left
        assert_eq!(pad_string("Ada", 6, Alignment::Right), "   Ada");
        // width <= len -> unchanged
        assert_eq!(pad_string("Ada", 3, Alignment::Right), "Ada");
        assert_eq!(pad_string("Ada", 2, Alignment::Right), "Ada");
    }

    #[test]
    fn pad_string_center_alignment_even_and_odd_pad() {
        // Even total pad: split evenly (2 left, 2 right)
        // len=3, width=7 -> pad=4 -> left=2 right=2
        assert_eq!(pad_string("Ada", 7, Alignment::Center), "  Ada  ");

        // Odd total pad: floor to left, ceil to right
        // len=3, width=6 -> pad=3 -> left=1 right=2
        assert_eq!(pad_string("Ada", 6, Alignment::Center), " Ada  ");
    }

    #[test]
    fn wrap_preserves_leading_spaces() {
        let s = "   foo";
        assert_eq!(wrap_ansi_aware(s, 80), "   foo");
    }

    #[test]
    fn wrap_preserves_trailing_spaces() {
        let s = "foo   ";
        let out = wrap_ansi_aware(s, 80);
        assert_eq!(out, "foo   ");
        assert!(out.ends_with("   "));
    }

    #[test]
    fn wrap_preserves_only_spaces_line() {
        let s = "      ";
        assert_eq!(wrap_ansi_aware(s, 80), "      ");
    }

    #[test]
    fn wrap_preserves_spaces_across_lines() {
        let s = "   foo  \n  bar ";
        let out = wrap_ansi_aware(s, 80);
        assert_eq!(out, "   foo  \n  bar ");
    }
}
