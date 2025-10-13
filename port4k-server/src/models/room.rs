use std::collections::HashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::Row;
use uuid::Uuid;
use crate::db::DbResult;
use crate::db::error::DbError;
use crate::models::json_string_vec_opt;
use crate::models::types::{AccountId, BlueprintId, Direction, ObjectId, RoomId, ZoneId};

static OBJ_REF_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\{obj:([a-zA-Z0-9_\- ]+)}").unwrap());

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
    pub scripts_inline: Vec<String>,
}

impl BlueprintRoom {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let hints = json_string_vec_opt(row.try_get::<_, Option<Value>>("hints")?, "hints")?;
        let scripts_inline = json_string_vec_opt(row.try_get::<_, Option<Value>>("scripts")?, "scripts")?;

        Ok(BlueprintRoom {
            id: RoomId(row.try_get::<_, Uuid>("id")?),
            bp_id: BlueprintId(row.try_get::<_, Uuid>("bp_id")?),
            key: row.try_get("key")?,
            title: row.try_get("title")?,
            body: row.try_get("body")?,
            lockdown: row.try_get("lockdown")?,
            short: row.try_get("short")?,
            hints,
            scripts_inline,
        })
    }
}


/// Row model for `bp_exits`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomExitRow {
    pub from_room_id: RoomId,
    pub dir: Direction,
    pub to_room_id: RoomId,
    pub locked: bool,
    pub description: Option<String>,
    // When locked, is this exit visible to players?
    pub visible_when_locked: bool,
}

impl RoomExitRow {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let dir_s: String = row.try_get("dir")?;
        let dir = Direction::parse(&dir_s)
            .ok_or_else(|| DbError::Decode(format!("invalid direction in bp_exits: {}", dir_s)))?;

        Ok(Self {
            from_room_id: row.try_get("from_room_id")?,
            dir,
            to_room_id: row.try_get("to_room_id")?,
            locked: row.try_get("locked")?,
            description: row.try_get("description")?,
            visible_when_locked: row.try_get("visible_when_locked")?,
        })
    }
}

/// Row model for `bp_objects`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomObjectRow {
    /// The ID of the object
    pub id: ObjectId,
    /// Room the object resides in
    pub room_id: RoomId,
    /// Name of the object
    pub name: String,
    /// Short description (one-liner)
    pub short: String,
    /// Full description
    pub description: String,
    /// Examine texts (if any)
    pub examine: Option<String>,
    /// State strings (arbitrary JSON array of strings)
    pub state: Vec<String>,
    /// Additional properties (arbitrary JSON map)
    pub props: Option<Value>,
    /// Lua script to run when `use`
    pub use_lua: Option<String>,
    /// Position for ordering (optional)
    pub position: Option<i32>,
}

impl RoomObjectRow {
    #[allow(unused)]
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let state = json_string_vec_opt(row.try_get::<_, Option<Value>>("state")?, "state")?;
        Ok(Self {
            id: row.try_get("id")?,
            room_id: row.try_get("room_id")?,
            name: row.try_get("name")?,
            short: row.try_get("short")?,
            description: row.try_get("description")?,
            examine: row.try_get("examine")?,
            state,
            use_lua: row.try_get("use_lua")?,
            position: row.try_get("position")?,
            props: row.try_get("props")?, // stays JSON
        })
    }
}

/// Noun mapping for objects (`bp_object_nouns`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectNounRow {
    pub room_id: RoomId,
    pub obj_id: ObjectId,
    pub noun: String,
}

impl ObjectNounRow {
    #[allow(unused)]
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        Ok(Self {
            room_id: row.try_get("room_id")?,
            obj_id: row.try_get("obj_id")?,
            noun: row.try_get("noun")?,
        })
    }
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
    pub account_id: AccountId,
    pub room_id: RoomId,
    pub coins: i32,
    pub health: i32,
    pub xp: i32,
    pub current_room: Option<RoomId>,
    pub room_qty: HashMap<ObjectId, i32>,
    pub trail: Vec<(RoomId, RoomId)>,
}



/// ZoneObjectState overlay for objects in a room in a zone.
pub struct ZoneObjectState {
    /// Zone in which the room lives
    pub zone_id: ZoneId,
    /// Room id in which the object lives
    pub room_id: RoomId,
    /// Object id we overlay
    pub obj_id: ObjectId,
    /// Quantity (if applicable)
    pub qty: Option<i32>,
    /// Any overlay flags (if any)
    pub flags: Vec<String>,
    /// additional arbitrary JSON data (if any)
    pub extra: Option<Value>,
}

/// `bp_room_kv` & `bp_player_kv` shapes at runtime.
pub type RoomKv = HashMap<String, Vec<String>>;
#[allow(unused)]
pub type PlayerKv = HashMap<String, Vec<String>>; // flattened per player; usually fetched later for a specific account

/// Runtime-friendly object with resolved nouns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomObject {
    pub id: ObjectId,
    pub name: String,
    pub short: String,
    pub description: String,
    pub examine: Option<String>,
    pub use_lua: Option<String>,
    pub position: Option<i32>,
    pub state: Vec<String>,
    pub nouns: Vec<String>,

    // Overlay
    pub initial_qty: Option<i32>,
    pub qty: Option<i32>,
    pub locked: bool,
    pub revealed: bool,
    pub takeable: bool,
    pub stackable: bool,
    pub is_coin: bool,
}

impl RoomObject {
    fn has_flag(list: &[String], flag: &str) -> bool {
        list.iter().any(|s| s.eq_ignore_ascii_case(flag))
    }

    #[allow(unused)]
    fn is_visible(&self) -> bool {
        !self.locked || self.revealed
    }
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
        let lockdown = self.room.lockdown;
        self.exits.iter().filter(move |e| {
            if lockdown { e.visible_when_locked } else { !e.locked || e.visible_when_locked }
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
        OBJ_REF_RE.replace_all(&self.room.body, |caps: &regex::Captures| {
            let key = &caps[1];
            self.object_by_noun(key)
                .map(|o| o.short.as_str())
                .unwrap_or(key)
                .to_string()
        }).into_owned()
    }

    pub fn with_overlay(mut self, overlay: &[ZoneObjectState]) -> Self {
        let by_id: HashMap<ObjectId, &ZoneObjectState> = overlay.iter().map(|z| (z.obj_id, z)).collect();

        for o in &mut self.objects {
            if let Some(z) = by_id.get(&o.id) {
                // qty
                if z.qty.is_some() { o.qty = z.qty; }

                o.locked = o.locked || RoomObject::has_flag(&z.flags, "locked");
                o.revealed = o.revealed || RoomObject::has_flag(&z.flags, "revealed");
            } else {
                o.qty = o.initial_qty;
            }
        }

        self.objects.retain(|o| !o.stackable || o.qty.unwrap_or(0) > 0);

        self
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // --- helpers -------------------------------------------------------------

    fn br() -> BlueprintRoom {
        BlueprintRoom {
            id: RoomId(Uuid::new_v4()),
            bp_id: BlueprintId(Uuid::new_v4()),
            key: "entry".into(),
            title: "Entry Hall".into(),
            body: "A dim corridor. {obj:coin} glints in the dust. {obj:unknown}".into(),
            lockdown: false,
            short: Some("A dim corridor".into()),
            hints: vec![],
            scripts_inline: vec![],
        }
    }

    fn exit(from: RoomId, to: RoomId, locked: bool, visible_when_locked: bool) -> RoomExitRow {
        RoomExitRow {
            from_room_id: from,
            dir: Direction::parse("east").expect("valid dir"),
            to_room_id: to,
            locked,
            description: Some("A steel door".into()),
            visible_when_locked,
        }
    }

    fn obj(
        name: &str,
        nouns: &[&str],
        short: &str,
        description: &str,
        takeable: bool,
        stackable: bool,
        is_coin: bool,
        initial_qty: Option<i32>,
        locked: bool,
        revealed: bool,
        qty: Option<i32>,
    ) -> RoomObject {
        RoomObject {
            id: ObjectId(Uuid::new_v4()),
            name: name.into(),
            short: short.into(),
            description: description.into(),
            examine: None,
            use_lua: None,
            position: None,
            state: vec![], // blueprint flags already reflected in the booleans below
            nouns: nouns.iter().map(|s| s.to_string()).collect(),
            initial_qty,
            qty,
            locked,
            revealed,
            takeable,
            stackable,
            is_coin,
        }
    }

    fn base_view_with(objects: Vec<RoomObject>, exits: Vec<RoomExitRow>) -> RoomView {
        RoomView {
            room: br(),
            exits,
            objects,
            scripts: RoomScripts::default(),
            room_kv: HashMap::new(),
            zone_state: None,
        }
    }

    // --- tests ---------------------------------------------------------------

    #[test]
    fn visible_exits_respects_locked_and_visibility_flag() {
        let r_from = RoomId(Uuid::new_v4());
        let r_to = RoomId(Uuid::new_v4());
        let exits = vec![
            exit(r_from, r_to, false, false), // unlocked → visible
            exit(r_from, r_to, true, false),  // locked + not visible_when_locked → hidden
            exit(r_from, r_to, true, true),   // locked + visible_when_locked → visible
        ];

        let view = base_view_with(vec![], exits);
        let visible: Vec<&RoomExitRow> = view.visible_exits().collect();
        assert_eq!(visible.len(), 2, "only two exits should be visible");
        assert!(visible.iter().any(|e| !e.locked));
        assert!(visible.iter().any(|e| e.locked && e.visible_when_locked));
    }

    #[test]
    fn object_by_noun_matches_name_and_synonyms_case_insensitive() {
        let o1 = obj(
            "Blast Door",
            &["door", "gate"],
            "a heavy blast door",
            "It looks sealed tight.",
            false, false, false,
            None, false, false, None,
        );
        let o2 = obj(
            "coin",
            &["credits", "money"],
            "a shiny coin",
            "A small minted coin.",
            true, true, true,
            Some(10), false, true, Some(10),
        );

        let view = base_view_with(vec![o1.clone(), o2.clone()], vec![]);

        assert!(view.object_by_noun("door").is_some());
        assert!(view.object_by_noun("BLAST DOOR").is_some());
        assert!(view.object_by_noun("credits").is_some());
        assert!(view.object_by_noun("money").is_some());
        assert!(view.object_by_noun("nope").is_none());
    }

    #[test]
    fn render_body_replaces_known_object_refs_and_leaves_unknowns() {
        let coin = obj(
            "coin",
            &["credits"],
            "a shiny coin",
            "A small minted coin.",
            true, true, true,
            Some(10), false, true, Some(10),
        );
        let view = base_view_with(vec![coin], vec![]);
        let body = view.render_body_with_object_refs();

        assert!(
            body.contains("a shiny coin"),
            "should replace {{obj:coin}} with object's short"
        );
        assert!(
            body.contains("unknown"),
            "unknown refs should remain as the raw key"
        );
    }

    #[test]
    fn with_overlay_applies_qty_and_flags_and_filters_zero_stackables() {
        let coin = obj(
            "coin",
            &["credits"],
            "a shiny coin",
            "A small minted coin.",
            true,  /* takeable */
            true,  /* stackable */
            true,  /* is_coin */
            Some(10), /* initial_qty */
            false, /* locked */
            false, /* revealed */
            None,  /* qty (will be set) */
        );

        let wrench = obj(
            "wrench",
            &["tool"],
            "a sturdy wrench",
            "Useful for bolts.",
            true,  /* takeable */
            false, /* stackable */
            false, /* is_coin */
            None,  /* initial_qty */
            false,
            false,
            None,
        );

        let mut view = base_view_with(vec![coin.clone(), wrench.clone()], vec![]);

        // Overlay sets coin to qty=0 and marks it revealed; wrench locked
        let overlay = vec![
            ZoneObjectState {
                zone_id: ZoneId(Uuid::new_v4()),
                room_id: view.room.id,
                obj_id: coin.id,
                qty: Some(0),
                flags: vec!["revealed".into()],
                extra: None,
            },
            ZoneObjectState {
                zone_id: ZoneId(Uuid::new_v4()),
                room_id: view.room.id,
                obj_id: wrench.id,
                qty: None,
                flags: vec!["locked".into()],
                extra: None,
            },
        ];

        view = view.with_overlay(&overlay);

        // coin is stackable and qty=0 → filtered out
        assert!(
            view.objects.iter().all(|o| o.name != "coin"),
            "stackable objects with qty=0 should be hidden"
        );

        // wrench should remain and be locked
        let w = view.objects.iter().find(|o| o.name == "wrench").expect("wrench present");
        assert!(w.locked, "wrench should be locked via overlay flag");
        assert!(w.is_visible(), "non-stackable locked object is still considered visible per is_visible()");
    }

    #[test]
    fn with_overlay_seeds_qty_from_initial_when_overlay_missing() {
        let coin = obj(
            "coin",
            &["credits"],
            "a shiny coin",
            "A small minted coin.",
            true,
            true,
            true,
            Some(10), // initial qty defined in blueprint
            false,
            false,
            None,     // no explicit qty yet
        );

        let view = base_view_with(vec![coin], vec![]);
        let merged = view.with_overlay(&[]); // no overlay for this object

        let c = merged.objects.iter().find(|o| o.name == "coin").expect("coin present");
        assert_eq!(c.qty, Some(10), "qty should be seeded from initial_qty when overlay is absent");
    }
}
