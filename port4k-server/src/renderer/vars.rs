use crate::Session;
use crate::models::room::RoomView;
use crate::renderer::RenderVars;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Returns a list of variables available for rendering templates.
pub fn generate_render_vars(sess: Arc<RwLock<Session>>, rv: Option<&RoomView>) -> RenderVars {
    RenderVars {
        global: get_global_vars(sess.clone()),
        room_view: rv.map(get_roomview_vars).unwrap_or_default(),
    }
}

fn get_global_vars(sess: Arc<RwLock<Session>>) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Generic vars not tied to account or location
    vars.insert(
        "wall_time".to_string(),
        chrono::Local::now().format("%H:%M:%S").to_string(),
    );
    vars.insert(
        "online_time".to_string(),
        format!("{}", sess.read().session_started.elapsed().as_secs()),
    );
    vars.insert("online_users".to_string(), format!("{}", 123));
    vars.insert("unread_messages".to_string(), format!("{}", 0));
    vars.insert("active_quests".to_string(), format!("{}", 0));
    vars.insert("now_utc".to_string(), chrono::Utc::now().to_rfc3339());
    vars.insert("now_local".to_string(), chrono::Local::now().to_rfc3339());

    if let Some(account) = sess.read().account.as_ref() {
        vars.insert("account.name".to_string(), account.username.to_string());
        vars.insert("account.role".to_string(), account.role.to_string());
        vars.insert("account.xp".to_string(), format!("{}", account.xp));
        vars.insert("account.health".to_string(), format!("{}", account.health));
        vars.insert("account.coins".to_string(), format!("{}", account.coins));
    }
    if let Some(cursor) = sess.read().cursor.as_ref() {
        vars.insert("cursor.zone".to_string(), cursor.zone_ctx.zone.title.to_string());
        vars.insert("cursor.room.title".to_string(), cursor.room_view.room.title.to_string());
        // vars.insert("cursor.view".to_string(), cursor.room.title.to_string());
    }

    vars
}

// Emits:
//   <prefix>.<key>            -> first value (if any)
//   <prefix>.<key>.all        -> "a, b, c"
//   <prefix>.<key>.count      -> N
//   <prefix>.<key>.<i>        -> ith value
//   <prefix>.<key>.has.<val>  -> "1" for presence (val is slugged)
// (from: get_roomview_vars)
fn emit_kv_list(
    vars: &mut HashMap<String, String>,
    prefix: &str,
    key: &str,
    values: &[String],
) {
    let sk = slug(key);
    let base = format!("{prefix}.{sk}");
    push(vars, &format!("{base}.count"), values.len());

    if let Some(first) = values.get(0) {
        push(vars, &base, first);
    }

    push(vars, &format!("{base}.all"), join_list(values));

    for (i, v) in values.iter().enumerate() {
        push(vars, &format!("{base}.{i}"), v);
    }

    // Set-style presence flags (unique)
    let mut uniq = HashSet::new();
    for v in values {
        let vv = slug(v);
        if uniq.insert(vv.clone()) {
            push(vars, &format!("{base}.has.{vv}"), "1");
        }
    }
}

#[inline] // (from: get_roomview_vars)
fn push(vars: &mut HashMap<String, String>, key: &str, val: impl ToString) {
    vars.insert(key.to_string(), val.to_string());
}

#[inline] // (from: get_roomview_vars)
fn yesno(b: bool) -> &'static str { if b { "true" } else { "false" } }

#[inline]
fn join_list(vs: &[String]) -> String {
    if vs.is_empty() { "none".to_string() } else { vs.join(", ") }
}

/// Turn names into safe, stable keys: "Blast Door" -> "blast_door"
/// (from: get_roomview_vars)
fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || ch == '-' {
            if !out.ends_with('_') { out.push('_'); }
        }
        // skip other punctuation entirely
    }
    // trim any trailing underscores
    while out.ends_with('_') { out.pop(); }
    if out.is_empty() { "obj".to_string() } else { out }
}

// --- main (from: get_roomview_vars) ------------------------------------------

pub fn get_roomview_vars(rv: &RoomView) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Room basics
    push(&mut vars, "title", &rv.room.title);
    push(&mut vars, "body",  &rv.room.body);

    // --------------------
    // Exits (aggregate)
    // --------------------
    let mut exit_dirs: Vec<String> = rv.exits.iter().map(|e| e.direction.to_string()).collect();
    exit_dirs.sort(); // stable output for deterministic templates

    let exits_line = if exit_dirs.is_empty() { "none".to_string() } else { exit_dirs.join(", ") };
    push(&mut vars, "exits_line", &exits_line);
    push(&mut vars, "exits",      &exits_line); // keep your original alias
    push(&mut vars, "exit_count", exit_dirs.len());
    push(&mut vars, "has_exits",  yesno(!exit_dirs.is_empty()));

    // Per-exit presence flags like exit.north.present=1
    for d in &exit_dirs {
        push(&mut vars, &format!("exit.{}.present", d.to_lowercase()), "1");
    }

    // --------------------
    // Objects (aggregate)
    // --------------------
    let mut all_objs: Vec<String> = rv.objects.iter().map(|o| o.name.to_string()).collect();
    all_objs.sort();

    let items_line = if all_objs.is_empty() { "none".to_string() } else { all_objs.join(", ") };
    push(&mut vars, "items_line", &items_line);
    push(&mut vars, "items",      &items_line); // keep your original alias
    push(&mut vars, "item_count", all_objs.len());
    push(&mut vars, "has_items",  yesno(!all_objs.is_empty()));

    let mut visible_objs: Vec<String> = rv
        .objects
        .iter()
        .filter(|o| o.flags.is_visible())
        .map(|o| o.name.to_string())
        .collect();
    visible_objs.sort();

    let visible_line = if visible_objs.is_empty() { "none".to_string() } else { visible_objs.join(", ") };
    push(&mut vars, "visible_items_line", &visible_line);
    push(&mut vars, "visible_items",      &visible_line);
    push(&mut vars, "visible_item_count", visible_objs.len());
    push(&mut vars, "has_visible_items",  yesno(!visible_objs.is_empty()));

    for o in &rv.objects {
        // let key = slug(&o.name);
        let key = o.name.to_string();
        push(&mut vars, &format!("obj.{}.name", key), &o.name);
        push(&mut vars, &format!("obj.{}.short", key), &o.short);
        push(&mut vars, &format!("obj.{}.description", key), &o.description);
        push(&mut vars, &format!("obj.{}.examine", key), o.examine.as_deref().unwrap_or("You see nothing special."));
        push(&mut vars, &format!("obj.{}.visible", key), yesno(o.flags.is_visible()));
        push(&mut vars, &format!("obj.{}.quantity", key), o.qty);
        push(&mut vars, &format!("obj.{}.locked", key), yesno(o.flags.locked));
        push(&mut vars, &format!("obj.{}.revealed", key), yesno(o.flags.revealed));
        push(&mut vars, &format!("obj.{}.takeable", key), yesno(o.flags.takeable));
        push(&mut vars, &format!("obj.{}.stackable", key), yesno(o.flags.stackable));
        push(&mut vars, &format!("obj.{}.is_coin", key), yesno(o.is_coin));
    }

    // --------------------
    // room_kv passthrough (namespaced)
    // --------------------
    // for (k, vs) in rv.room_kv.iter() {
    //     emit_kv_list(&mut vars, "room.kv", k, vs.to_slice());
    // }

    // push(&mut vars, "state.present", yesno(rv.zone_state.is_some()));
    // push(&mut vars, "is_empty_room", yesno(exit_dirs.is_empty() && all_objs.is_empty()));

    vars
}
