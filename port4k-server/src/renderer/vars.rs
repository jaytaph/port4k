use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::models::room::RoomView;
use crate::renderer::RenderVars;
use crate::Session;

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
    vars.insert("wall_time".to_string(), chrono::Local::now().format("%H:%M:%S").to_string());
    vars.insert("online_time".to_string(), format!("{}", sess.read().session_started.elapsed().as_secs()));
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

fn get_roomview_vars(rv: &RoomView) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("title".to_string(), rv.room.title.to_string());
    vars.insert("body".to_string(), rv.room.body.to_string());

    // Add exits
    let dirs: Vec<String> = rv.exits.iter().map(|e| e.dir.to_string()).collect();
    let exits_line = if dirs.is_empty() {
        "none".to_string()
    } else {
        dirs.join(", ")
    };
    vars.insert("exits_line".to_string(), exits_line);
    vars.insert("exits".to_string(), rv.exits.iter().map(|e| e.dir.to_string()).collect::<Vec<String>>().join(", "));

    // Add objects
    let objs: Vec<String> = rv.objects.iter().map(|o| o.name.to_string()).collect();
    let objs_line = if objs.is_empty() {
        "none".to_string()
    } else {
        objs.join(", ")
    };
    vars.insert("objects_line".to_string(), objs_line);
    vars.insert("objects".to_string(), rv.objects.iter().map(|o| o.name.to_string()).collect::<Vec<String>>().join(", "));

    vars
}
