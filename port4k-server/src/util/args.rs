use crate::db::repo::room::BlueprintAndRoomKey;

/// Utility functions for argument parsing
pub fn parse_bp_room_key(s: &str) -> Option<BlueprintAndRoomKey> {
    let (bp_key, room_key) = s.split_once(':')?;
    if bp_key.is_empty() || room_key.is_empty() {
        return None;
    }

    Some(BlueprintAndRoomKey::new(bp_key, room_key))
}

pub fn normalize_dir(s: &str) -> Option<&'static str> {
    match s.trim().to_ascii_lowercase().as_str() {
        "n" | "north"        => Some("north"),
        "ne" | "northeast"   => Some("northeast"),
        "e" | "east"         => Some("east"),
        "se" | "southeast"   => Some("southeast"),
        "s" | "south"        => Some("south"),
        "sw" | "southwest"   => Some("southwest"),
        "w" | "west"         => Some("west"),
        "nw" | "northwest"   => Some("northwest"),
        "u" | "up"           => Some("up"),
        "d" | "down"         => Some("down"),
        _ => None,
    }
}


// pub fn split_args_quoted(s: &str) -> Vec<String> {
//     let mut out = Vec::new();
//     let mut cur = String::new();
//     let mut in_q = false;
//     let mut chars = s.chars().peekable();
//     while let Some(ch) = chars.next() {
//         match ch {
//             '"' => in_q = !in_q,
//             ' ' | '\t' if !in_q => {
//                 if !cur.is_empty() {
//                     out.push(std::mem::take(&mut cur));
//                 }
//             }
//             '\\' => {
//                 if let Some(&next) = chars.peek() {
//                     if next == '"' || next == '\\' {
//                         cur.push(next);
//                         chars.next();
//                     } else {
//                         cur.push(ch);
//                     }
//                 } else {
//                     cur.push(ch);
//                 }
//             }
//             _ => cur.push(ch),
//         }
//     }
//     if !cur.is_empty() {
//         out.push(cur);
//     }
//     out
// }
