use serde_json::Value;
use crate::db::DbResult;
use crate::db::error::DbError;
use crate::models::room::{Kv, KvResolved};

pub fn json_string_vec_opt(v: Option<Value>, field: &'static str) -> DbResult<Vec<String>> {
    match v {
        None => Ok(Vec::new()),
        Some(val) => {
            let out: Vec<String> = serde_json::from_value(val).map_err(|_| DbError::Decode(field.into()))?;
            Ok(out)
        }
    }
}
//
// pub fn json_to_vec_strings(v: Value) -> AppResult<Vec<String>> {
//     // This enforces: string  OR  array-of-strings. Nothing else.
//     let parsed: StrOrVec = serde_json::from_value(v)?;
//     match parsed {
//         StrOrVec::Str(s) => Ok(vec![s]),
//         StrOrVec::Vec(v) => {
//             // (Optional) validate no nulls inside (serde already guarantees String)
//             Ok(v)
//         }
//     }
// }

/// Normalize a JSON value into Vec<String>.
/// - ["a","b"]      -> vec!["a","b"]
/// - "a"            -> vec!["a"]
/// - 123 / true     -> vec!["123"] / vec!["true"]
/// - null / {} / [] -> vec![]
pub fn json_to_string_vec(v: &Value) -> Vec<String> {
    match v {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|x| {
                x.as_str().map(|s| s.to_string()).or_else(|| {
                    // accept scalars inside the array (numbers/bools) by stringify
                    match x {
                        Value::Number(_) | Value::Bool(_) => Some(x.to_string()),
                        _ => None,
                    }
                })
            })
            .collect(),
        Value::String(s) => vec![s.clone()],
        Value::Number(_) | Value::Bool(_) => vec![v.to_string()],
        _ => Vec::new(),
    }
}


#[inline]
pub fn resolve_qty(bp_default: i32, zone_override: Option<i32>, user_override: Option<i32>) -> i32 {
    let v = user_override.or(zone_override).unwrap_or(bp_default);
    v.max(0)
}

#[inline]
pub fn resolve_bool(bp_default: bool, zone: Option<bool>, user: Option<bool>) -> bool {
    user.or(zone).unwrap_or(bp_default)
}

#[inline]
pub fn kv_get_bool(kv: &KvResolved, key: &str, default: bool) -> bool {
    match kv.get(key) {
        Some(s) => str_is_truthy(s),
        None => default,
    }
}

#[inline]
pub fn str_is_truthy(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

// #[inline]
// pub fn kv_to_resolved(kv: &Kv) -> KvResolved {
//     let mut out = KvResolved::with_capacity(kv.inner.len());
//     for (k, v) in &kv.inner {
//         out.insert(k.clone(), str_or_vec_to_vec(v));
//     }
//     out
// }

#[inline]
pub fn merge_kv(bp: &Kv, zone: &Kv, user: &Kv) -> KvResolved {
    let mut out = KvResolved::new();
    for (k, v) in &bp.inner {
        out.insert(k.clone(), v.clone());
    }
    for (k, v) in &zone.inner {
        out.insert(k.clone(), v.clone());
    }
    for (k, v) in &user.inner {
        out.insert(k.clone(), v.clone());
    }
    out
}

#[inline]
pub fn compute_object_visible(kv: &KvResolved, revealed: bool) -> bool {
    let hidden = kv_get_bool(kv, "hidden", false);
    if hidden {
        return false;
    }
    let discovered = kv_get_bool(kv, "discovered", false);
    revealed || discovered
}

// pub fn passive_discovery(
//     rv: &RoomView,
//     zr: &mut ZoneRoomState,
//     perception: u8, // from character sheet; fall back to default
// ) -> Vec<ObjectId> {
//     let mut revealed = vec![];
//     for obj in &rv.objects {
//         if let Discovery::Obscured { dc } = obj.discovery
//             && perception >= dc
//             && zr.discovered_objects.insert(obj.id)
//         {
//             revealed.push(obj.id);
//         }
//     }
//     revealed
// }

// pub fn is_visible_to(obj: &RoomObject, rv: &RoomView, zr: &ZoneRoomState) -> bool {
//     if zr.discovered_objects.contains(&obj.id) {
//         return true;
//     }
//     match &obj.discovery {
//         Discovery::Visible => true,
//         Discovery::Hidden => false,
//         Discovery::Obscured { .. } => false, // until discovered via checks
//         Discovery::Conditional { key, value } => rv
//             .room_kv
//             .get(key)
//             .map(|vals| vals.iter().any(|v| v == value))
//             .unwrap_or(false),
//         Discovery::Scripted => false, // let Lua flip discovery when ready
//     }
// }
