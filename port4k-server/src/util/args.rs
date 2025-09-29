pub fn parse_bp_room_key(s: &str) -> Option<(String, String)> {
    let (bp, room) = s.split_once(':')?;
    if bp.is_empty() || room.is_empty() {
        return None;
    }
    Some((bp.to_string(), room.to_string()))
}

pub fn split_args_quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => in_q = !in_q,
            ' ' | '\t' if !in_q => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '\\' => {
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' {
                        cur.push(next);
                        chars.next();
                    } else {
                        cur.push(ch);
                    }
                } else {
                    cur.push(ch);
                }
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
