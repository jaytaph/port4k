use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::db::types::{BlueprintId, Direction, RoomId, ZoneId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintRoom {
    pub id: RoomId,
    pub bp_id: BlueprintId,
    pub key: String,
    pub title: String,
    pub body: String,
    pub lockdown: bool,
    pub short: Option<String>,
    pub hints: Vec<String>,
    pub scripts_inline: Vec<String>, // legacy column; prefer *_scripts tables
    // You’ll fetch attached data via joins: exits, objects, scripts, kv
}

/// Row model for `bp_exits`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomExitRow {
    pub from_room_id: RoomId,
    pub dir: Direction,
    pub to_room_id: RoomId,
    pub locked: bool,
    pub description: Option<String>,
    pub visible_when_locked: bool,
}

/// Row model for `bp_objects`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomObjectRow {
    pub id: Uuid,
    pub room_id: RoomId,
    pub name: String,
    pub short: String,
    pub description: String,
    pub examine: Option<String>,
    pub state: Vec<String>,
    pub use_lua: Option<String>,
    pub position: Option<i32>,
}

/// Noun mapping for objects (`bp_object_nouns`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectNounRow {
    pub room_id: RoomId,
    pub obj_id: Uuid,
    pub noun: String,
}

/// Scripts for a room (pulled from live or draft tables).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoomScripts {
    pub on_enter_lua: Option<String>,
    pub on_command_lua: Option<String>,
}

/// `zone_room_state` row merged into runtime view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneRoomState {
    pub zone_id: ZoneId,
    pub room_id: RoomId,
    pub state: Vec<String>, // arbitrary JSON map
}

/// `bp_room_kv` & `bp_player_kv` shapes at runtime.
pub type RoomKv = HashMap<String, Vec<String>>;
pub type PlayerKv = HashMap<String, Vec<String>>; // flattened per player; usually fetched later for a specific account

/// Runtime-friendly object with resolved nouns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomObject {
    pub id: Uuid,
    pub name: String,
    pub short: String,
    pub description: String,
    pub examine: Option<String>,
    pub use_lua: Option<String>,
    pub position: Option<i32>,
    pub state: Vec<String>,
    pub nouns: Vec<String>,
}

/// Runtime view the engine uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomView {
    pub room: BlueprintRoom,
    pub exits: Vec<RoomExitRow>,
    pub objects: Vec<RoomObject>,
    pub scripts: RoomScripts,
    pub room_kv: RoomKv,
    pub zone_state: Option<ZoneRoomState>,
}

impl RoomView {
    /// from fn: `RoomView::visible_exits`
    pub fn visible_exits(&self) -> impl Iterator<Item = &RoomExitRow> {
        self.exits.iter().filter(|e| {
            // If room is under lockdown, you could choose to hide exits entirely, or still show visible_when_locked ones.
            // Here we honor exit visibility rules:
            if e.locked {
                e.visible_when_locked
            } else {
                true
            }
        })
    }

    /// from fn: `RoomView::object_by_noun`
    pub fn object_by_noun(&self, noun: &str) -> Option<&RoomObject> {
        self.objects.iter().find(|o| {
            o.name.eq_ignore_ascii_case(noun) ||
                o.nouns.iter().any(|n| n.eq_ignore_ascii_case(noun))
        })
    }

    /// from fn: `RoomView::render_body_with_object_refs`
    /// Replaces `{obj:name}` with the object's `short` text.
    pub fn render_body_with_object_refs(&self) -> String {
        use regex::Regex;
        let re = Regex::new(r"\{obj:([a-zA-Z0-9_\- ]+)}").unwrap();
        re.replace_all(&self.room.body, |caps: &regex::Captures| {
            let key = caps.get(1).unwrap().as_str();
            if let Some(obj) = self.object_by_noun(key) {
                obj.short.to_string()
            } else {
                key.to_string() // fallback if not found
            }
        }).into_owned()
    }
}