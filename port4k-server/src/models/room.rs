use crate::db::DbResult;
use crate::db::error::DbError;
use crate::models::types::{BlueprintId, Direction, ObjectId, RoomId, ZoneId};
use crate::util::visibility::is_visible_to;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tokio_postgres::Row;
use uuid::Uuid;

static OBJ_REF_RE: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"\{obj:([a-zA-Z0-9_\- ]+)}").unwrap());


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    pub once: Option<bool>, // default: false
    pub text: String,
    pub when: String,        // first_look, enter, search, after_fail
    pub cooldown: Option<u32>, // seconds; null = no cooldown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintRoom {
    pub id: RoomId,
    pub bp_id: BlueprintId,
    pub key: String,
    pub title: String,
    pub body: String,
    pub lockdown: bool,
    pub short: Option<String>,
    pub hints: Vec<Hint>,
}

impl BlueprintRoom {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let hints_val: Option<Value> = row.try_get::<_, Option<Value>>("hints")?;
        let hints = parse_hints_value(hints_val)?;

        Ok(BlueprintRoom {
            id: RoomId(row.try_get::<_, Uuid>("id")?),
            bp_id: BlueprintId(row.try_get::<_, Uuid>("bp_id")?),
            key: row.try_get("key")?,
            title: row.try_get("title")?,
            body: row.try_get("body")?,
            lockdown: row.try_get("lockdown")?,
            short: row.try_get("short")?,
            hints,
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
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        // JSON state (NOT NULL in schema, default '{}')
        let raw_state: Value = row.try_get("state")?;

        // Policy:
        // - If it's an OBJECT -> treat it as props, leave state Vec<String> empty.
        // - Else (array/string/number/bool) -> normalize into Vec<String>, props None.
        let (state, props) = match &raw_state {
            Value::Object(_) => (Vec::new(), Some(raw_state.clone())),
            _ => (json_to_string_vec(&raw_state), None),
        };

        Ok(Self {
            id: ObjectId(row.try_get::<_, Uuid>("id")?),
            room_id: RoomId(row.try_get::<_, Uuid>("room_id")?),
            name: row.try_get("name")?,
            short: row.try_get("short")?,
            description: row.try_get("description")?,
            examine: row.try_get("examine")?,
            use_lua: row.try_get("use_lua")?,
            position: row.try_get("position")?,
            state,
            props,
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ZoneRoomState {
    // Zone ID we are in (is this needed)
    // pub zone_id: ZoneId,
    /// Account ID of the player whose state this is
    // pub account_id: AccountId,
    /// Room ID this state is for
    pub room_id: RoomId,

    // pub coins: i32, // @TODO: is this needed here or just in player state?
    // pub health: i32, // @TODO: is this needed here or just in player state?
    // pub xp: i32,  // @TODO: is this needed here or just in player state?

    // pub current_room: Option<RoomId>,
    /// Items in the room and their quantities, including hidden/undiscovered ones
    pub all_objects: HashMap<ObjectId, i32>,
    /// Objects that are visible for the player (this assumes that ALL quantities are visible at the same time)
    pub discovered_objects: HashSet<ObjectId>,
    // Trail of rooms the player has visited to get here (for backtracking)
    // pub trail: Vec<(RoomId, RoomId)>,
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

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum Discovery {
    #[default]
    Visible, // always listed
    Hidden, // never listed until discovered
    Obscured {
        dc: u8,
    }, // requires a perception check >= dc
    Conditional {
        key: String,
        value: String,
    }, // visible if room_kv[key]==value
    Scripted, // let Lua decide
}

/// Runtime-friendly object with resolved nouns.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomObject {
    /// Blueprint Object ID
    pub id: ObjectId,
    /// Name of the object (ie: "wrench")
    pub name: String,
    /// Short description (one-liner)
    pub short: String,
    /// Full description
    pub description: String,
    /// Examine text (if any)
    pub examine: Option<String>,
    /// Lua script to run when `use`
    pub use_lua: Option<String>,
    /// Position for ordering (optional)
    pub position: Option<i32>,
    /// State strings (arbitrary flags)
    pub state: HashMap<String, String>,
    /// Synonyms / alternate nouns (terminal, console, computer, screen)
    pub nouns: Vec<String>,
    /// How is this object discovered in the room?
    pub discovery: Discovery,

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

    pub fn is_visible(&self) -> bool {
        // visible objects are either non-stackable items, or revealed stackables
        !self.stackable || self.revealed
    }

    pub fn is_visible_to(&self, rv: &RoomView, zr: &ZoneRoomState) -> bool {
        let discovered = is_visible_to(self, rv, zr);
        discovered && self.is_visible()
    }

    pub fn from_rows(row_obj: &RoomObjectRow, nouns: &[String]) -> Self {
        Self {
            id: row_obj.id,
            name: row_obj.name.clone(),
            short: row_obj.short.clone(),
            description: row_obj.description.clone(),
            examine: row_obj.examine.clone(),
            use_lua: row_obj.use_lua.clone(),
            position: row_obj.position,
            state: row_obj.state.iter().map(|s| (s.clone(), "true".to_string())).collect(),
            nouns: nouns.as_ref().to_vec(),
            discovery: Discovery::Visible,

            initial_qty: None,
            qty: None,
            locked: false,
            revealed: false,
            takeable: false,
            stackable: false,
            is_coin: false,
        }
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
            if lockdown {
                e.visible_when_locked
            } else {
                !e.locked || e.visible_when_locked
            }
        })
    }

    /// from fn: `RoomView::object_by_noun`
    pub fn object_by_noun(&self, noun: &str) -> Option<&RoomObject> {
        self.objects
            .iter()
            .find(|o| o.name.eq_ignore_ascii_case(noun) || o.nouns.iter().any(|n| n.eq_ignore_ascii_case(noun)))
    }

    /// from fn: `RoomView::render_body_with_object_refs`
    /// Replaces `{obj:name}` with the object's `short` text.
    pub fn render_body_with_object_refs(&self) -> String {
        OBJ_REF_RE
            .replace_all(&self.room.body, |caps: &regex::Captures| {
                let key = &caps[1];
                self.object_by_noun(key)
                    .map(|o| o.short.as_str())
                    .unwrap_or(key)
                    .to_string()
            })
            .into_owned()
    }

    pub fn with_overlay(mut self, overlay: &[ZoneObjectState]) -> Self {
        let by_id: HashMap<ObjectId, &ZoneObjectState> = overlay.iter().map(|z| (z.obj_id, z)).collect();

        for o in &mut self.objects {
            if let Some(z) = by_id.get(&o.id) {
                // qty
                if z.qty.is_some() {
                    o.qty = z.qty;
                }

                o.locked = o.locked || RoomObject::has_flag(&z.flags, "locked");
                o.revealed = o.revealed || RoomObject::has_flag(&z.flags, "revealed");
            } else {
                o.qty = o.initial_qty;
            }
        }

        self.objects.retain(|o| !o.stackable || o.qty.unwrap_or(0) > 0);

        self
    }

    pub fn visible_objects<'a>(&'a self, zr: &'a ZoneRoomState) -> impl Iterator<Item = &'a RoomObject> + 'a {
        self.objects.iter().filter(|o| is_visible_to(o, self, zr))
    }
}

// Allowed 'when' values; tweak as you like
const ALLOWED_WHEN: &[&str] = &["first_look", "enter", "search", "after_fail", "manual"];

fn normalize_when<S: AsRef<str>>(s: S) -> String {
    let lower = s.as_ref().trim().to_ascii_lowercase().replace([' ', '-'], "_");
    if ALLOWED_WHEN.contains(&lower.as_str()) {
        lower
    } else {
        // conservative default: only show when explicitly asked (prevents spam)
        "manual".to_string()
    }
}

fn parse_hints_value(hints: Option<Value>) -> DbResult<Vec<Hint>> {
    let Some(v) = hints else { return Ok(Vec::new()); };

    match v {
        Value::Array(arr) => {
            if arr.iter().all(|x| x.is_string()) {
                // Legacy format: ["hint 1", "hint 2", ...]
                let out = arr.into_iter().filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .map(|text| Hint {
                        once: None,
                        text,
                        when: "manual".to_string(),
                        cooldown: None,
                    })
                    .collect();
                Ok(out)
            } else {
                // v2 format: array of objects
                // Deserialize then normalize 'when'
                let mut hints: Vec<Hint> = serde_json::from_value(Value::Array(arr))
                    .map_err(|e| DbError::Validation(format!("invalid hints array: {e}")))?;
                for h in &mut hints {
                    if h.when.trim().is_empty() {
                        h.when = "manual".to_string();
                    } else {
                        h.when = normalize_when(&h.when);
                    }
                }
                Ok(hints)
            }
        }
        Value::Object(_) => {
            // Accept a single object as shorthand
            let mut h: Hint = serde_json::from_value(v).map_err(|e| DbError::Validation(format!("invalid hint object: {e}")))?;
            if h.when.trim().is_empty() {
                h.when = "manual".to_string();
            } else {
                h.when = normalize_when(&h.when);
            }
            Ok(vec![h])
        }
        Value::Null => Ok(Vec::new()),
        other => Err(DbError::Validation(format!("unexpected JSON type for hints: {other:?}"))),
    }
}

// --- Helpers you can reuse anywhere ----------------------------------------

/// Normalize a JSON value into Vec<String>.
/// - ["a","b"]      -> vec!["a","b"]
/// - "a"            -> vec!["a"]
/// - 123 / true     -> vec!["123"] / vec!["true"]
/// - null / {} / [] -> vec![]
fn json_to_string_vec(v: &Value) -> Vec<String> {
    match v {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()).or_else(|| {
                // accept scalars inside the array (numbers/bools) by stringifying
                match x {
                    Value::Number(_) | Value::Bool(_) => Some(x.to_string()),
                    _ => None
                }
            }))
            .collect(),
        Value::String(s) => vec![s.clone()],
        Value::Number(_) | Value::Bool(_) => vec![v.to_string()],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // --- helpers -------------------------------------------------------------

    fn dummy_room_view() -> RoomView {
        RoomView {
            room: br(),
            exits: vec![],
            objects: vec![],
            scripts: RoomScripts::default(),
            zone_state: None,
            room_kv: HashMap::new(),
        }
    }

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
            // scripts_inline: vec![],
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

    #[allow(clippy::too_many_arguments)]
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
            state: HashMap::new(),
            nouns: nouns.iter().map(|s| s.to_string()).collect(),
            discovery: Discovery::Visible,
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
            false,
            false,
            false,
            None,
            false,
            false,
            None,
        );
        let o2 = obj(
            "coin",
            &["credits", "money"],
            "a shiny coin",
            "A small minted coin.",
            true,
            true,
            true,
            Some(10),
            false,
            true,
            Some(10),
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
            true,
            true,
            true,
            Some(10),
            false,
            true,
            Some(10),
        );
        let view = base_view_with(vec![coin], vec![]);
        let body = view.render_body_with_object_refs();

        assert!(
            body.contains("a shiny coin"),
            "should replace {{obj:coin}} with object's short"
        );
        assert!(body.contains("unknown"), "unknown refs should remain as the raw key");
    }

    #[test]
    fn with_overlay_applies_qty_and_flags_and_filters_zero_stackables() {
        let coin = obj(
            "coin",
            &["credits"],
            "a shiny coin",
            "A small minted coin.",
            true,     /* takeable */
            true,     /* stackable */
            true,     /* is_coin */
            Some(10), /* initial_qty */
            false,    /* locked */
            false,    /* revealed */
            None,     /* qty (will be set) */
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
        let w = view
            .objects
            .iter()
            .find(|o| o.name == "wrench")
            .expect("wrench present");
        assert!(w.locked, "wrench should be locked via overlay flag");
        assert!(
            w.is_visible(),
            "non-stackable locked object is still considered visible per is_visible()"
        );
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
            None, // no explicit qty yet
        );

        let view = base_view_with(vec![coin], vec![]);
        let merged = view.with_overlay(&[]); // no overlay for this object

        let c = merged.objects.iter().find(|o| o.name == "coin").expect("coin present");
        assert_eq!(
            c.qty,
            Some(10),
            "qty should be seeded from initial_qty when overlay is absent"
        );
    }

    #[test]
    fn non_stackable_hidden_then_visible_after_discovery() {
        // arrange
        let wrench_id = ObjectId::new(); // or ObjectId::from(...), etc.
        let wrench = RoomObject {
            id: wrench_id,
            name: "wrench".into(),
            stackable: false,
            revealed: false,              // ignored for non-stackables
            locked: true,                 // lock doesn't affect visibility
            discovery: Discovery::Hidden, // hidden until discovered
            ..Default::default()
        };

        let rv = dummy_room_view();
        let mut zr = ZoneRoomState {
            discovered_objects: HashSet::new(),
            ..Default::default()
        };

        // before discovery: not visible (discovery blocks)
        assert!(
            !wrench.is_visible_to(&rv, &zr),
            "non-stackable wrench should NOT be visible before discovery"
        );
        // intrinsic policy says it's renderable on its own
        assert!(
            wrench.is_visible(),
            "intrinsic visibility for non-stackables is true regardless of `revealed`/`locked`"
        );

        // act: player discovers the wrench (e.g., by examining something)
        zr.discovered_objects.insert(wrench_id);

        // after discovery: visible (discovery ∧ intrinsic)
        assert!(
            wrench.is_visible_to(&rv, &zr),
            "non-stackable wrench becomes visible once discovered"
        );
    }

    #[test]
    fn stackable_requires_revealed_even_if_discovered() {
        // arrange
        let coins_id = ObjectId::new();
        let coins = RoomObject {
            id: coins_id,
            name: "gold coin".into(),
            stackable: true,
            revealed: false, // covered/buried
            locked: false,
            discovery: Discovery::Visible, // even if globally visible...
            ..Default::default()
        };
        let rv = dummy_room_view();
        let mut zr = ZoneRoomState {
            discovered_objects: HashSet::new(),
            ..Default::default()
        };
        zr.discovered_objects.insert(coins_id); // discovered

        // stackables still need `revealed=true` to render
        assert!(
            !coins.is_visible_to(&rv, &zr),
            "stackable items remain hidden until `revealed=true`, even if discovered"
        );

        // act: uncover the pile
        let mut coins_uncovered = coins.clone();
        coins_uncovered.revealed = true;

        assert!(
            coins_uncovered.is_visible_to(&rv, &zr),
            "stackable items become visible once revealed"
        );
    }
}