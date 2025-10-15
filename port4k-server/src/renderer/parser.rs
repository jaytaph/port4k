use std::collections::HashSet;

/// Internal token representation
#[derive(Clone, Debug)]
pub enum Token {
    Text(String),
    /// Any {rv:...} variable; kept separate for possible special handling
    RoomVar {
        raw: String,             // original token text for LeaveToken
        name: String,
        default: Option<String>,
        fmt: Option<VarFmt>,
    },
    /// Any {v:...} variable
    Var {
        raw: String,             // original token text for LeaveToken
        name: String,
        default: Option<String>,
        fmt: Option<VarFmt>,
    },
    /// Any {c:...} color spec
    Color {
        fg: Option<String>,
        bg: Option<String>,
        attrs: Vec<String>,
    },
    ColorReset,
    Unknown(String),             // pass-through
}

#[derive(Clone, Debug)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub enum VarFmt {
    // %[-]Ns
    String { width: Option<u32>, alignment: Alignment },
    // %0Nd or %Nd
    Int { width: Option<u32>, zero_pad: bool },
}

/// Parse a full template into tokens.
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
                // flush pending text
                if !buf.is_empty() {
                    out.push(Token::Text(std::mem::take(&mut buf)));
                }
                // read token until matching '}'
                if let Some((raw, content, next_i)) = read_braced(input, i) {
                    i = next_i;
                    out.push(parse_token(raw, content));
                } else {
                    // no closing '}', treat as literal
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

// Read {...} starting at '{'; return (raw_token, inner_content, next_index)
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
    // content can be:  "v:var", "v:var:default", "v:var|%05d", "v:var:default|%-20s"
    // colors: "c" or "c:yellow", "c:yellow:red:bold,underline", etc.
    let mut parts = content.splitn(2, ':');
    let kind = parts.next().unwrap_or("");
    let rest = parts.next();

    match kind {
        "rv" => parse_var(raw, rest.unwrap_or(""), true),
        "v" => parse_var(raw, rest.unwrap_or(""), false),
        "c" => parse_color(raw, rest),
        _ => Token::Unknown(raw),
    }
}

fn parse_var(raw: String, rest: &str, is_room_var: bool) -> Token {
    // we need to split possible trailing |%fmt from the right
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
    let name = parts.next().unwrap_or("").trim().to_string();
    let default = parts.next().map(|s| s.to_string());

    if is_room_var {
        return Token::RoomVar { raw, name, default, fmt };
    }
    Token::Var { raw, name, default, fmt }
}

fn parse_color(_raw: String, rest: Option<&str>) -> Token {
    match rest {
        None | Some("") => Token::ColorReset,
        Some(spec) => {
            // Accept: fg[:bg][:attr[,attr...]]  OR fg[:attr[,attr...]]
            let mut items: Vec<&str> = spec.split(':').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if items.is_empty() {
                return Token::ColorReset;
            }

            let fg: Option<String>;
            let mut bg: Option<String> = None;
            let mut attrs: Vec<String> = Vec::new();

            // always parse first as fg
            fg = Some(items.remove(0).to_string());

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
    a.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).map(|x| x.to_string()).collect()
}

fn is_color_name(s: &str) -> bool {
    // keep in sync with ansi.rs names
    const NAMES: [&str; 24] = [
        "black","red","green","yellow","blue","magenta","cyan","white","gray","grey",
        "bright_black","bright_red","bright_green","bright_yellow","bright_blue","bright_magenta","bright_cyan","bright_white",
        "default","reset",
        "orange","purple","teal","pink", // optional extensions if you map them
    ];
    let set: HashSet<&'static str> = HashSet::from(NAMES);
    set.contains(&s)
}

fn parse_format(spec: &str) -> Option<VarFmt> {
    // accepts: %s, %20s, %-20s, %d, %05d, %5d
    if !spec.starts_with('%') { return None; }
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