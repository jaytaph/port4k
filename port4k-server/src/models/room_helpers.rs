use crate::models::room::{Kv, KvResolved};

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
        Some(s) => s.as_bool().unwrap_or(false),
        None => default,
    }
}

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
