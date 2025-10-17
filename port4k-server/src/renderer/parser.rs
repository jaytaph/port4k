use std::collections::{HashMap, HashSet};

/// ===============================
/// Token & formatting definitions
/// ===============================

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VarFmt {
    // %[-|*]Ns -> left or center aligned string; right aligned by default
    String { width: Option<u32>, alignment: Alignment },
    // %0Nd or %Nd
    Int { width: Option<u32>, zero_pad: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,  // {v:...}
    Room,    // {rv:...}
    Object,  // {o:...}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VarToken {
    pub raw: String,              // original "{...}"
    pub scope: Scope,
    pub key: String,              // for o: "door.short" or just "door"
    pub default: Option<String>,
    pub fmt: Option<VarFmt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugScope { All, Global, Room, Object }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugKey {
    None, // whole scope
    Var(String),                          // v/rv
    Object(String),                       // o:name
    ObjectProp { name: String, prop: String }, // o:name.prop
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugToken {
    pub raw: String,
    pub scope: DebugScope,
    pub key: DebugKey,
    pub fmt: Option<VarFmt>, // applied to VALUE for single-item cases
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Text(String),
    /// {c:...}
    Color { fg: Option<String>, bg: Option<String>, attrs: Vec<String> },
    ColorReset,
    /// {v:..}/{rv:..}/{o:..}
    Var(VarToken),
    /// {dbg[:...]}
    Debug(DebugToken),
    /// Unknown or passthrough. We keep raw so author can see literal.
    Unknown(String),
}

/// ===============================
/// Minimal Room model (for {o:...})
/// ===============================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomObject {
    pub name: String,
    pub short: String,
    pub locked: bool,
    pub revealed: bool,
    /// Optional additional properties for tests / extensions
    pub props: HashMap<String, String>,
}

impl RoomObject {
    // (from user’s earlier logic)
    pub fn is_visible(&self) -> bool /* from: impl RoomObject::is_visible */ {
        // visible when not locked OR when revealed
        !self.locked || self.revealed
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomView {
    pub objects: Vec<RoomObject>,
}

/// ===============================
/// Rendering context
/// ===============================

pub struct Ctx<'a> {
    pub designer_mode: bool,
    pub globals: &'a HashMap<String, String>,   // {v:...}
    pub room_vars: &'a HashMap<String, String>, // {rv:...}
    pub room_view: &'a RoomView,                 // {o:...}
}

/// ===============================
/// Parser
/// ===============================

/// Parse a full template into tokens. (from: parse)
pub fn parse(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    let b = input.as_bytes();

    let mut buf = String::new();

    while i < b.len() {
        match b[i] {
            b'{' => {
                // handle "{{" escape
                if i + 1 < b.len() && b[i + 1] == b'{' {
                    buf.push('{');
                    i += 2;
                    continue;
                }
                // try to read a full braced token FIRST
                if let Some((raw, content, next_i)) = read_braced(input, i) {
                    // only now flush pending text
                    if !buf.is_empty() {
                        out.push(Token::Text(std::mem::take(&mut buf)));
                    }
                    i = next_i;
                    out.push(parse_token(raw, content));
                } else {
                    // no closing '}', treat '{' literally without flushing buf
                    buf.push('{');
                    i += 1;
                }
            }
            b'}' => {
                // handle "}}" escape
                if i + 1 < b.len() && b[i + 1] == b'}' {
                    buf.push('}');
                    i += 2;
                } else {
                    buf.push('}');
                    i += 1;
                }
            }
            _ => {
                buf.push(b[i] as char);
                i += 1;
            }
        }
    }
    if !buf.is_empty() {
        out.push(Token::Text(buf));
    }
    out
}

// Read {...} starting at '{'; return (raw_token, inner_content, next_index) (from: read_braced)
fn read_braced(input: &str, start: usize) -> Option<(String, String, usize)> {
    let bytes = input.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'}' {
            let raw = &input[start..=i];
            let inner = &input[start + 1..i];
            return Some((raw.to_string(), inner.to_string(), i + 1));
        }
        i += 1;
    }
    None
}

// (from: parse_token)
fn parse_token(raw: String, content: String) -> Token {
    // content can be: "v:var", "v:var:default", "v:var|%05d", "v:var:default|%-20s"
    // colors: "c" or "c:yellow", "c:yellow:red:bold,underline", etc.
    // debug: "dbg", "dbg:v", "dbg:o:wrench", "dbg:rv:user", ...
    let mut parts = content.splitn(2, ':');
    let tag = parts.next().unwrap_or("");
    let rest = parts.next();

    match tag {
        "v"  => parse_var_like(raw, rest.unwrap_or(""), Scope::Global),
        "rv" => parse_var_like(raw, rest.unwrap_or(""), Scope::Room),
        "o"  => parse_var_like(raw, rest.unwrap_or(""), Scope::Object),
        "c"  => parse_color(raw, rest),
        "dbg"=> parse_dbg(raw, rest.unwrap_or("")),
        _    => Token::Unknown(raw),
    }
}

// Parse {v:...}/{rv:...}/{o:...} (from: parse_var_like)
fn parse_var_like(raw: String, rest: &str, scope: Scope) -> Token {
    // split possible trailing |%fmt from the right
    let mut vv = rest.rsplitn(2, '|');
    let right = vv.next().unwrap_or("");
    let left = vv.next(); // everything before the last '|', if any

    let (core, fmt) = if let Some(left_side) = left {
        (left_side, parse_format(right))
    } else if right.starts_with('%') {
        ("", parse_format(right))
    } else {
        (right, None)
    };

    // Now split core on ':' into name[:default]
    let mut parts = core.splitn(2, ':');
    let key = parts.next().unwrap_or("").trim().to_string();
    let default = parts.next().map(|s| s.to_string());

    Token::Var(VarToken { raw, scope, key, default, fmt })
}

// Parse {c:...} (from: parse_color)
fn parse_color(_raw: String, rest: Option<&str>) -> Token {
    match rest {
        None | Some("") => Token::ColorReset,
        Some(spec) => {
            // Accept: fg[:bg][:attr[,attr...]]  OR fg[:attr[,attr...]]
            let mut items: Vec<&str> = spec.split(':').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if items.is_empty() {
                return Token::ColorReset;
            }

            let mut bg: Option<String> = None;
            let mut attrs: Vec<String> = Vec::new();

            // always parse first as fg
            let fg: Option<String> = Some(items.remove(0).to_string());

            if !items.is_empty() {
                // heuristic: if the next section contains ',', it's attrs; otherwise treat it as bg or attrs
                let next = items.remove(0).to_string();
                if next.contains(',') {
                    attrs = split_attrs(&next);
                } else {
                    // could be bg name OR attr name; decide by membership
                    let is_color_like = is_color_name(&next);
                    if is_color_like {
                        bg = Some(next);
                    } else {
                        attrs = split_attrs(&next);
                    }
                }
            }

            // any remaining part is attrs (possibly multiple)
            for a in items {
                for v in a.split(',') {
                    let v = v.trim();
                    if !v.is_empty() {
                        attrs.push(v.to_string());
                    }
                }
            }

            Token::Color { fg, bg, attrs }
        }
    }
}

fn split_attrs(a: &str) -> Vec<String> {
    a.split(',')
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .map(|x| x.to_string())
        .collect()
}

// small, static-ish color name set (from: is_color_name)
fn is_color_name(s: &str) -> bool {
    static NAMES: once_cell::sync::Lazy<HashSet<&'static str>> = once_cell::sync::Lazy::new(|| {
        HashSet::from([
            "black","red","green","yellow","blue","magenta","cyan","white",
            "gray","grey","bright_black","bright_red","bright_green","bright_yellow",
            "bright_blue","bright_magenta","bright_cyan","bright_white",
            "default","reset","orange","purple","teal","pink",
        ])
    });
    NAMES.contains(s)
}

// Parse {dbg[:...]} (from: parse_dbg)
fn parse_dbg(raw: String, rest: &str) -> Token {
    // allow optional |%fmt for single-item debug
    let (core, fmt) = {
        let mut vv = rest.rsplitn(2, '|');
        let right = vv.next().unwrap_or("");
        let left = vv.next();
        if let Some(l) = left {
            (l, parse_format(right))
        } else if right.starts_with('%') {
            ("", parse_format(right))
        } else {
            (right, None)
        }
    };

    if core.is_empty() {
        return Token::Debug(DebugToken { raw, scope: DebugScope::All, key: DebugKey::None, fmt });
    }

    let mut segs = core.split(':');
    match (segs.next(), segs.next()) {
        (Some("v"),  None)         => Token::Debug(DebugToken { raw, scope: DebugScope::Global, key: DebugKey::None, fmt }),
        (Some("rv"), None)         => Token::Debug(DebugToken { raw, scope: DebugScope::Room,   key: DebugKey::None, fmt }),
        (Some("o"),  None)         => Token::Debug(DebugToken { raw, scope: DebugScope::Object, key: DebugKey::None, fmt }),
        (Some("v"),  Some(k))      => Token::Debug(DebugToken { raw, scope: DebugScope::Global, key: DebugKey::Var(k.to_string()), fmt }),
        (Some("rv"), Some(k))      => Token::Debug(DebugToken { raw, scope: DebugScope::Room,   key: DebugKey::Var(k.to_string()), fmt }),
        (Some("o"),  Some(k))      => {
            if let Some((n,p)) = k.split_once('.') {
                Token::Debug(DebugToken { raw, scope: DebugScope::Object, key: DebugKey::ObjectProp { name: n.to_string(), prop: p.to_string() }, fmt })
            } else {
                Token::Debug(DebugToken { raw, scope: DebugScope::Object, key: DebugKey::Object(k.to_string()), fmt })
            }
        }
        _ => Token::Unknown(raw),
    }
}

// (from: parse_format)
fn parse_format(spec: &str) -> Option<VarFmt> {
    // accepts: %s, %20s, %-20s, %*20s, %d, %05d, %5d
    if !spec.starts_with('%') {
        return None;
    }
    let bytes = spec.as_bytes();
    let mut i = 1;

    let mut alignment = Alignment::Right;
    let mut zero = false;

    if i < bytes.len() && bytes[i] == b'-' {
        alignment = Alignment::Left;
        i += 1;
    } else if i < bytes.len() && bytes[i] == b'*' {
        alignment = Alignment::Center;
        i += 1;
    } else if i < bytes.len() && bytes[i] == b'0' {
        zero = true;
        i += 1;
    }

    // width digits
    let mut width: u32 = 0;
    let mut has_width = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        has_width = true;
        width = width * 10 + (bytes[i] - b'0') as u32;
        i += 1;
    }

    if i >= bytes.len() {
        return None;
    }
    let ty = bytes[i] as char;

    match ty {
        's' => Some(VarFmt::String {
            width: if has_width { Some(width) } else { None },
            alignment,
        }),
        'd' => Some(VarFmt::Int {
            width: if has_width { Some(width) } else { None },
            zero_pad: zero,
        }),
        _ => None,
    }
}

/// ===============================
/// Tests
/// ===============================
#[cfg(test)]
mod tests {
    use super::*;

    // ------------ helpers for assertions -------------
    fn unwrap_var(t: &Token) -> &VarToken {
        match t {
            Token::Var(v) => v,
            _ => panic!("expected Token::Var, got {:?}", t),
        }
    }

    fn unwrap_color(t: &Token) -> (Option<String>, Option<String>, Vec<String>) {
        match t {
            Token::Color { fg, bg, attrs } => (fg.clone(), bg.clone(), attrs.clone()),
            Token::ColorReset => (None, None, vec![]),
            _ => panic!("expected Token::Color or ColorReset, got {:?}", t),
        }
    }

    fn unwrap_dbg(t: &Token) -> &DebugToken {
        match t {
            Token::Debug(d) => d,
            _ => panic!("expected Token::Debug, got {:?}", t),
        }
    }

    // =================================================
    // from: RoomObject::is_visible
    // =================================================
    #[test]
    fn room_object_is_visible_rules() {
        let a = RoomObject { name: "door".into(), short: "A door".into(), locked: false, revealed: false, props: HashMap::new() };
        let b = RoomObject { name: "panel".into(), short: "A panel".into(), locked: true, revealed: false, props: HashMap::new() };
        let c = RoomObject { name: "safe".into(), short: "A safe".into(), locked: true, revealed: true, props: HashMap::new() };

        assert!(a.is_visible(), "unlocked -> visible");
        assert!(!b.is_visible(), "locked & not revealed -> hidden");
        assert!(c.is_visible(), "locked but revealed -> visible");
    }

    // =================================================
    // from: read_braced
    // =================================================
    #[test]
    fn read_braced_basic() {
        let s = "xx{hello}yy";
        let (raw, inner, next) = read_braced(s, 2).expect("should parse");
        assert_eq!(raw, "{hello}");
        assert_eq!(inner, "hello");
        assert_eq!(next, 9);
    }

    #[test]
    fn read_braced_no_close() {
        let s = "{unterminated";
        assert!(read_braced(s, 0).is_none());
    }

    // =================================================
    // from: parse_format
    // =================================================
    #[test]
    fn parse_format_string_alignments() {
        // %s -> String, no width, Right
        let f = parse_format("%s").unwrap();
        assert_eq!(f, VarFmt::String { width: None, alignment: Alignment::Right });

        // %-20s -> Left
        let f = parse_format("%-20s").unwrap();
        assert_eq!(f, VarFmt::String { width: Some(20), alignment: Alignment::Left });

        // %*10s -> Center
        let f = parse_format("%*10s").unwrap();
        assert_eq!(f, VarFmt::String { width: Some(10), alignment: Alignment::Center });
    }

    #[test]
    fn parse_format_int_width_zero_padding() {
        // %d
        let f = parse_format("%d").unwrap();
        assert_eq!(f, VarFmt::Int { width: None, zero_pad: false });

        // %5d
        let f = parse_format("%5d").unwrap();
        assert_eq!(f, VarFmt::Int { width: Some(5), zero_pad: false });

        // %05d
        let f = parse_format("%05d").unwrap();
        assert_eq!(f, VarFmt::Int { width: Some(5), zero_pad: true });
    }

    #[test]
    fn parse_format_rejects_unknown() {
        assert!(parse_format("%q").is_none());
        assert!(parse_format("not-a-format").is_none());
    }

    // =================================================
    // from: split_attrs
    // =================================================
    #[test]
    fn split_attrs_trims_and_skips_empty() {
        assert_eq!(split_attrs("bold,  underline ,, blink "), vec!["bold","underline","blink"]);
        assert!(split_attrs("").is_empty());
    }

    // =================================================
    // from: is_color_name
    // =================================================
    #[test]
    fn is_color_name_knowns_and_unknowns() {
        assert!(is_color_name("red"));
        assert!(is_color_name("bright_cyan"));
        assert!(is_color_name("default"));
        assert!(!is_color_name("chartreuse42"));
    }

    // =================================================
    // from: parse_color
    // =================================================
    #[test]
    fn parse_color_reset_and_fg_only() {
        // {c} or {c:}
        let t = parse_color("{c}".into(), None);
        match t { Token::ColorReset => {}, _ => panic!("expected ColorReset") }

        let t = parse_color("{c:red}".into(), Some("red"));
        let (fg, bg, attrs) = unwrap_color(&t);
        assert_eq!(fg, Some("red".into()));
        assert_eq!(bg, None);
        assert!(attrs.is_empty());
    }

    #[test]
    fn parse_color_fg_bg_attrs() {
        // fg:bg:attr1,attr2
        let t = parse_color("{c:yellow:red:bold,underline}".into(), Some("yellow:red:bold,underline"));
        let (fg, bg, attrs) = unwrap_color(&t);
        assert_eq!(fg.as_deref(), Some("yellow"));
        assert_eq!(bg.as_deref(), Some("red"));
        assert_eq!(attrs, vec!["bold","underline"]);

        // fg:attrs (when second isn't a color name)
        let t = parse_color("{c:yellow:bold,underline}".into(), Some("yellow:bold,underline"));
        let (fg, bg, attrs) = unwrap_color(&t);
        assert_eq!(fg.as_deref(), Some("yellow"));
        assert_eq!(bg, None);
        assert_eq!(attrs, vec!["bold","underline"]);

        // fg:bg:attr ... :attr (more parts)
        let t = parse_color("{c:blue:black:bold:italic,blink}".into(), Some("blue:black:bold:italic,blink"));
        let (fg, bg, attrs) = unwrap_color(&t);
        assert_eq!(fg.as_deref(), Some("blue"));
        assert_eq!(bg.as_deref(), Some("black"));
        assert_eq!(attrs, vec!["bold","italic","blink"]);
    }

    // =================================================
    // from: parse_var_like
    // =================================================
    #[test]
    fn parse_var_like_key_default_and_fmt() {
        // v:user
        let t = parse_var_like("{v:user}".into(), "user", Scope::Global);
        let v = unwrap_var(&t);
        assert_eq!(v.scope, Scope::Global);
        assert_eq!(v.key, "user");
        assert!(v.default.is_none());
        assert!(v.fmt.is_none());

        // v:user:guest
        let t = parse_var_like("{v:user:guest}".into(), "user:guest", Scope::Global);
        let v = unwrap_var(&t);
        assert_eq!(v.key, "user");
        assert_eq!(v.default.as_deref(), Some("guest"));

        // rv:score|%05d
        let t = parse_var_like("{rv:score|%05d}".into(), "score|%05d", Scope::Room);
        let v = unwrap_var(&t);
        assert_eq!(v.scope, Scope::Room);
        assert_eq!(v.key, "score");
        assert_eq!(v.fmt, Some(VarFmt::Int { width: Some(5), zero_pad: true }));

        // o:door.short|%*10s  (centered width 10)
        let t = parse_var_like("{o:door.short|%*10s}".into(), "door.short|%*10s", Scope::Object);
        let v = unwrap_var(&t);
        assert_eq!(v.scope, Scope::Object);
        assert_eq!(v.key, "door.short");
        assert_eq!(v.fmt, Some(VarFmt::String { width: Some(10), alignment: Alignment::Center }));

        // edge: only fmt present -> empty key but fmt parsed
        let t = parse_var_like("{v:|%5d}".into(), "|%5d", Scope::Global);
        let v = unwrap_var(&t);
        assert_eq!(v.key, "");
        assert_eq!(v.fmt, Some(VarFmt::Int { width: Some(5), zero_pad: false }));
    }

    // =================================================
    // from: parse_dbg
    // =================================================
    #[test]
    fn parse_dbg_variants() {
        // dbg (all scopes dump)
        let t = parse_dbg("{dbg}".into(), "");
        let d = unwrap_dbg(&t);
        assert!(matches!(d.scope, DebugScope::All));
        assert!(matches!(d.key, DebugKey::None));
        assert!(d.fmt.is_none());

        // dbg:v (globals)
        let t = parse_dbg("{dbg:v}".into(), "v");
        let d = unwrap_dbg(&t);
        assert!(matches!(d.scope, DebugScope::Global));
        assert!(matches!(d.key, DebugKey::None));

        // dbg:o:wrench
        let t = parse_dbg("{dbg:o:wrench}".into(), "o:wrench");
        let d = unwrap_dbg(&t);
        assert!(matches!(d.scope, DebugScope::Object));
        match &d.key {
            DebugKey::Object(n) => assert_eq!(n, "wrench"),
            _ => panic!("expected Object"),
        }

        // dbg:o:wrench.short
        let t = parse_dbg("{dbg:o:wrench.short}".into(), "o:wrench.short");
        let d = unwrap_dbg(&t);
        match &d.key {
            DebugKey::ObjectProp { name, prop } => {
                assert_eq!(name, "wrench");
                assert_eq!(prop, "short");
            }
            _ => panic!("expected ObjectProp"),
        }

        // dbg:v:user|%20s (fmt on single value)
        let t = parse_dbg("{dbg:v:user|%20s}".into(), "v:user|%20s");
        let d = unwrap_dbg(&t);
        assert_eq!(d.fmt, Some(VarFmt::String { width: Some(20), alignment: Alignment::Right }));
    }

    // =================================================
    // from: parse_token
    // =================================================
    #[test]
    fn parse_token_routes_to_specific_parsers() {
        // v / rv / o
        let t = parse_token("{v:user}".into(), "v:user".into());
        assert!(matches!(t, Token::Var(VarToken { scope: Scope::Global, .. })));
        let t = parse_token("{rv:room}".into(), "rv:room".into());
        assert!(matches!(t, Token::Var(VarToken { scope: Scope::Room, .. })));
        let t = parse_token("{o:door}".into(), "o:door".into());
        assert!(matches!(t, Token::Var(VarToken { scope: Scope::Object, .. })));

        // color
        let t = parse_token("{c:red}".into(), "c:red".into());
        match t { Token::Color { .. } => {}, _ => panic!("expected color") }

        // dbg
        let t = parse_token("{dbg:o:wrench}".into(), "dbg:o:wrench".into());
        match t { Token::Debug(_) => {}, _ => panic!("expected debug") }

        // unknown
        let t = parse_token("{foo:bar}".into(), "foo:bar".into());
        match t { Token::Unknown(r) => assert_eq!(r, "{foo:bar}"), _ => panic!("expected unknown") }
    }

    // =================================================
    // from: parse (top-level tokenizer)
    // =================================================
    #[test]
    fn parse_text_and_tokens_and_escapes() {
        let s = "Hi {{ user }} -> {v:user}, color {c:yellow}ok{c}";
        let toks = parse(s);
        // Expect: "Hi { user } -> " TEXT, Var, TEXT, Color, TEXT, ColorReset
        assert!(matches!(toks[0], Token::Text(ref t) if t.contains("Hi { user } -> ")));
        assert!(matches!(toks[1], Token::Var(_)));
        assert!(matches!(toks[2], Token::Text(ref t) if t.starts_with(", color ")));
        assert!(matches!(toks[3], Token::Color{..}));
        assert!(matches!(toks[4], Token::Text(ref t) if t == "ok"));
        assert!(matches!(toks[5], Token::ColorReset));
    }

    #[test]
    fn parse_unclosed_brace_treated_as_literal() {
        let s = "start {oops end";
        let toks = parse(s);
        dbg!(&toks);
        // Should remain a single Text token containing the literal
        assert_eq!(toks.len(), 1);
        assert_eq!(&toks[0], &Token::Text("start {oops end".into()));
    }
}