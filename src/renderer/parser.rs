use std::collections::HashSet;

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
    Global, // {v:...}
    Room,   // {rv:...}
    Object, // {o:...}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VarToken {
    pub raw: String, // original "{...}"
    pub scope: Scope,
    pub key: String, // for o: "door.short" or just "door"
    pub default: Option<String>,
    pub fmt: Option<VarFmt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugScope {
    All,
    Global,
    Room,
    Object,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugKey {
    None,                                      // whole scope
    Var(String),                               // v/rv
    Object(String),                            // o:name
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
    Color {
        fg: Option<String>,
        bg: Option<String>,
        attrs: Vec<String>,
    },
    ColorReset,
    /// {v:..}/{rv:..}/{o:..}
    Var(VarToken),
    /// {dbg[:...]}
    Debug(DebugToken),
    /// Unknown or passthrough. We keep raw so author can see literal.
    Unknown(String),
}

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

fn parse_token(raw: String, content: String) -> Token {
    // content can be: "v:var", "v:var:default", "v:var|%05d", "v:var:default|%-20s"
    // colors: "c" or "c:yellow", "c:yellow:red:bold,underline", etc.
    // debug: "dbg", "dbg:v", "dbg:o:wrench", "dbg:rv:user", ...
    let mut parts = content.splitn(2, ':');
    let tag = parts.next().unwrap_or("");
    let rest = parts.next();

    match tag {
        // (global) variables
        "v" | "var" => parse_var_like(raw, rest.unwrap_or(""), Scope::Global),
        // Room variables
        "rv" | "room" => parse_var_like(raw, rest.unwrap_or(""), Scope::Room),
        // Objects
        "o" | "obj" | "object" => parse_var_like(raw, rest.unwrap_or(""), Scope::Object),
        // Colors
        "c" | "col" | "color" => parse_color(raw, rest),
        // Debug
        "dbg" | "debug" => parse_dbg(raw, rest.unwrap_or("")),
        _ => Token::Unknown(raw),
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

    Token::Var(VarToken {
        raw,
        scope,
        key,
        default,
        fmt,
    })
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
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "white",
            "gray",
            "grey",
            "bright_black",
            "bright_red",
            "bright_green",
            "bright_yellow",
            "bright_blue",
            "bright_magenta",
            "bright_cyan",
            "bright_white",
            "default",
            "reset",
            "orange",
            "purple",
            "teal",
            "pink",
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
        return Token::Debug(DebugToken {
            raw,
            scope: DebugScope::All,
            key: DebugKey::None,
            fmt,
        });
    }

    let mut segs = core.split(':');
    match (segs.next(), segs.next()) {
        (Some("v"), None) | (Some("var"), None) => Token::Debug(DebugToken {
            raw,
            scope: DebugScope::Global,
            key: DebugKey::None,
            fmt,
        }),
        (Some("rv"), None) | (Some("room"), None) => Token::Debug(DebugToken {
            raw,
            scope: DebugScope::Room,
            key: DebugKey::None,
            fmt,
        }),
        (Some("o"), None) | (Some("obj"), None) | (Some("object"), None) => Token::Debug(DebugToken {
            raw,
            scope: DebugScope::Object,
            key: DebugKey::None,
            fmt,
        }),

        (Some("v"), Some(k)) | (Some("var"), Some(k)) => Token::Debug(DebugToken {
            raw,
            scope: DebugScope::Global,
            key: DebugKey::Var(k.to_string()),
            fmt,
        }),
        (Some("rv"), Some(k)) | (Some("room"), Some(k)) => Token::Debug(DebugToken {
            raw,
            scope: DebugScope::Room,
            key: DebugKey::Var(k.to_string()),
            fmt,
        }),
        (Some("o"), Some(k)) | (Some("obj"), Some(k)) | (Some("object"), Some(k)) => {
            if let Some((n, p)) = k.split_once('.') {
                Token::Debug(DebugToken {
                    raw,
                    scope: DebugScope::Object,
                    key: DebugKey::ObjectProp {
                        name: n.to_string(),
                        prop: p.to_string(),
                    },
                    fmt,
                })
            } else {
                Token::Debug(DebugToken {
                    raw,
                    scope: DebugScope::Object,
                    key: DebugKey::Object(k.to_string()),
                    fmt,
                })
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
