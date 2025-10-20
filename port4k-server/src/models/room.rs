use crate::db::DbResult;
use crate::db::error::DbError;
use crate::models::types::{BlueprintId, Direction, ObjectId, RoomId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio_postgres::Row;
use uuid::Uuid;
use crate::models::room_helpers::{compute_object_visible, merge_kv, resolve_bool, resolve_qty, str_is_truthy};
use crate::util::serde::serde_to_str;

/// Hints that a user can request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    pub once: Option<bool>,     // Only show once
    pub text: String,
    pub when: String,           // first_look, enter, search, after_fail
    pub cooldown: Option<u32>,  // seconds; null = no cooldown
}

/// Blueprint room model for `bp_rooms`. There are no zone or user overlays in here
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

/// Blueprint exit model. Note these are not reciprocal; each exit is one-way.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintExit {
    /// From Room ID
    pub from_room_id: RoomId,
    /// From Room Key
    pub from_room_key: String,
    /// Direction to go to
    pub dir: Direction,
    /// To Room ID
    pub to_room_id: RoomId,
    /// To Room Key
    pub to_room_key: String,
    /// Description of the exit
    pub description: Option<String>,
    /// Is the exit visible when locked?
    pub visible_when_locked: bool,
    /// Is the exit locked by default?
    pub default_locked: bool,
}

impl BlueprintExit {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let dir_s: String = row.try_get("dir")?;
        let dir = Direction::parse(&dir_s)
            .ok_or_else(|| DbError::Decode(format!("invalid direction in bp_exits: {}", dir_s)))?;

        Ok(Self {
            from_room_id: row.try_get("from_room_id")?,
            from_room_key: row.try_get("from_room_key")?,
            dir,
            to_room_id: row.try_get("to_room_id")?,
            to_room_key: row.try_get("to_room_key")?,
            description: row.try_get("description")?,
            visible_when_locked: row.try_get("visible_when_locked")?,
            default_locked: row.try_get("locked")?,
        })
    }
}

/// Blueprint object model for `bp_objects`. There are no zone or user overlays in here
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintObject {
    /// The ID of the object
    pub id: ObjectId,
    /// Name of the object (ie: "wrench")
    pub name: String,
    /// Short description (one-liner)
    pub short: String,
    /// Full description
    pub description: String,
    /// Examine texts (if any)
    pub examine: Option<String>,
    /// Lua script to run when `use`
    pub on_use_lua: Option<String>,
    /// Position for ordering (optional)
    pub position: Option<i32>,
    /// Synonyms / alternate nouns (terminal, console, computer, screen)
    pub nouns: Vec<String>,
    /// How is this object discovered in the room?
    // pub discovery: Discovery,

    /// Object key values
    pub object_kv: Kv,
    /// Initial quantity of the object (if stackable)
    pub initial_qty: Option<i32>,
    /// Is the object locked by default?
    pub default_locked: bool,
    /// Is the object revealed by default?
    pub default_revealed: bool,
    /// Is the object takeable?
    pub takeable: bool,
    /// Is the object stackable?
    pub stackable: bool,
    /// Is the object a coin/currency?
    pub is_coin: bool,
}

impl BlueprintObject {
    pub fn try_from_row(row: &Row, nouns: Vec<String>, kv: Kv) -> DbResult<Self> {
        Ok(Self {
            id: ObjectId(row.try_get::<_, Uuid>("id")?),
            name: row.try_get("name")?,
            short: row.try_get("short")?,
            description: row.try_get("description")?,
            examine: row.try_get("examine")?,
            on_use_lua: row.try_get("use_lua")?,
            position: row.try_get("position")?,
            nouns,
            // discovery: Discovery::default(),

            // This needs to be set
            object_kv: kv,
            initial_qty: None,
            default_locked: false,
            default_revealed: false,
            takeable: false,
            stackable: false,
            is_coin: false,
        })
    }
}

/// Blueprint LUA room scripts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoomScripts {
    pub on_enter_lua: Option<String>,
    pub on_command_lua: Option<String>,
}

/// Resolved KV values. They are basically the same as the Kv type, but we know this type is
/// already resolved.
pub type KvResolved = HashMap<String, String>;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Kv {
    pub inner: HashMap<String, String>,
}

impl Kv {
    pub fn try_from_rows(rows: &[Row]) -> DbResult<Self> {
        let mut kv = Kv::default();
        for row in rows {
            let key: String = row.try_get("key")?;
            let value: String = row.try_get("value")?;
            kv.inner.insert(key, value);
        }
        Ok(kv)
    }

    pub fn from(data: Value) -> Kv {
        let mut kv = Kv::default();

        if let Value::Object(map) = data {
            for (k, v) in map {
                kv.inner.insert(k, serde_to_str(v));
            }
        }

        kv
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.inner.get(key)
    }
}

// #[derive(Clone, Debug, Serialize, Deserialize, Default)]
// pub enum Discovery {
//     #[default]
//     Visible, // always listed
//     Hidden, // never listed until discovered
//     Obscured { dc: u8 }, // requires a perception check >= dc
//     Conditional { key: String, value: String }, // visible if room_kv[key]==value
//     Scripted, // let Lua decide
// }



/// Runtime view the engine uses. It contains all resolved data for the specific zone and user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomView {
    pub room: BlueprintRoom,
    pub room_kv: KvResolved,

    pub exits: Vec<ResolvedExit>,
    pub exits_by_dir: HashMap<Direction, usize>,

    pub objects: Vec<ResolvedObject>,
    pub objects_by_key: HashMap<String, usize>,

    pub scripts: RoomScripts,
}

impl RoomView {
    // pub fn visible_exits(&self) -> impl Iterator<Item = &ResolvedExit> {
    //     let lockdown = self.room.lockdown;
    //     self.exits.iter().filter(move |e| {
    //         if lockdown {
    //             e.visible_when_locked
    //         } else {
    //             !e.locked || e.visible_when_locked
    //         }
    //     })
    // }

    pub fn object_by_noun(&self, noun: &str) -> Option<&ResolvedObject> {
        self.objects
            .iter()
            .find(|o| o.name.eq_ignore_ascii_case(noun) || o.nouns.iter().any(|n| n.eq_ignore_ascii_case(noun)))
    }
}


/// Builds up a complete room view by assembling blueprint, zone, and user data.
pub(crate) fn build_room_view_impl(
    bp_room: &BlueprintRoom,
    bp_exits: &[BlueprintExit],
    bp_objs: &[BlueprintObject],
    bp_room_kv: &Kv,

    zone_room_kv: &Kv,
    zone_obj_kv: &HashMap<String, Kv>,
    zone_qty_override: &HashMap<String, i32>,

    user_room_kv: &Kv,
    user_obj_kv: &HashMap<String, Kv>,
    user_qty_override: &HashMap<String, i32>,
) -> RoomView {
    let room_kv = merge_kv(bp_room_kv, zone_room_kv, user_room_kv);

    let mut exits = Vec::new();
    let mut exits_by_dir = HashMap::new();
    for e in bp_exits {

        // We store exit information inside the room KVs
        // exit.north.locked = true means the exit to the north is locked
        let key_locked = format!("exit.{}.locked", e.dir);
        let locked = resolve_bool(
            e.default_locked,
            zone_room_kv.get(&key_locked).map(String::as_str).map(str_is_truthy),
            user_room_kv.get(&key_locked).map(String::as_str).map(str_is_truthy),
        );

        let key_visible = format!("exit.{}.visible", e.dir);
        let visible = resolve_bool(
            // Note that this depends on the computed locked state from above
            !locked || e.visible_when_locked,
            zone_room_kv.get(&key_visible).map(String::as_str).map(str_is_truthy),
            user_room_kv.get(&key_visible).map(String::as_str).map(str_is_truthy),
        );

        let idx = exits.len();
        exits.push(ResolvedExit {
            direction: e.dir.clone(),
            from_room_id: e.from_room_id,
            from_room_key: e.from_room_key.clone(),
            to_room_id: e.to_room_id,
            to_room_key: e.to_room_key.clone(),
            flags: ExitFlags {
                locked,
                hidden: !visible,
                visible_when_locked: e.visible_when_locked,
            },
        });
        exits_by_dir.insert(e.dir.clone(), idx);
    }

    let mut objects = Vec::new();
    let mut objects_by_key = HashMap::new();
    for o in bp_objs {
        let key = o.name.clone();
        let kv = merge_kv(
            &o.object_kv,
            zone_obj_kv.get(&key).unwrap_or(&Kv::default()),
            user_obj_kv.get(&key).unwrap_or(&Kv::default())
        );
        let qty = resolve_qty(
            o.initial_qty.unwrap_or(1),
            zone_qty_override.get(&key).copied(),
            user_qty_override.get(&key).copied(),
        );
        let locked = resolve_bool(
            o.default_locked,
            zone_obj_kv
                .get(&key)
                .and_then(|kv| kv.get("locked"))
                .map(|v| str_is_truthy(v.as_str())),
            user_obj_kv
                .get(&key)
                .and_then(|kv| kv.get("locked"))
                .map(|v| str_is_truthy(v.as_str())),
        );

        let revealed = resolve_bool(
            o.default_revealed,
            zone_obj_kv
                .get(&key)
                .and_then(|kv| kv.get("revealed"))
                .map(|v| str_is_truthy(v.as_str())),
            user_obj_kv
                .get(&key)
                .and_then(|kv| kv.get("revealed"))
                .map(|v| str_is_truthy(v.as_str())),
        );
        let visible = compute_object_visible(&kv, revealed);

        objects_by_key.insert(key.clone(), objects.len());

        objects.push(ResolvedObject {
            id: o.id,
            key: key.clone(),
            name: o.name.clone(),
            short: o.short.clone(),
            description: o.description.clone(),
            examine: o.examine.clone(),
            use_lua: o.on_use_lua.clone(),
            nouns: o.nouns.clone(),
            position: o.position,
            kv,
            qty,
            flags: ObjectFlags {
                locked,
                hidden: !visible,
                revealed,
                takeable: o.takeable,
                stackable: o.stackable,
            },
            is_coin: o.is_coin,
        });
    }

    RoomView {
        room: bp_room.clone(),
        room_kv,
        exits,
        exits_by_dir,
        objects,
        objects_by_key,
        scripts: RoomScripts::default()
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
    let Some(v) = hints else {
        return Ok(Vec::new());
    };

    match v {
        Value::Array(arr) => {
            if arr.iter().all(|x| x.is_string()) {
                // Legacy format: ["hint 1", "hint 2", ...]
                let out = arr
                    .into_iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
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
            let mut h: Hint =
                serde_json::from_value(v).map_err(|e| DbError::Validation(format!("invalid hint object: {e}")))?;
            if h.when.trim().is_empty() {
                h.when = "manual".to_string();
            } else {
                h.when = normalize_when(&h.when);
            }
            Ok(vec![h])
        }
        Value::Null => Ok(Vec::new()),
        other => Err(DbError::Validation(format!(
            "unexpected JSON type for hints: {other:?}"
        ))),
    }
}

/// Exit flags
#[derive(Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ExitFlags {
    pub locked: bool,               // Exit is locked and cannot be passed
    pub hidden: bool,               // Exit is invisible to the player
    pub visible_when_locked: bool,  // Exit is visible even when locked
}

impl ExitFlags {
    pub fn is_visible(&self) -> bool {
        !self.hidden
    }
}

#[derive(Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ObjectFlags {
    pub locked: bool,       // Can't interact until unlocked
    pub hidden: bool,       // Invisible to the player if true
    pub revealed: bool,     // Has been discovered by the player
    pub takeable: bool,     // Can be picked up
    pub stackable: bool,    // Can be stacked in inventory (coins etc.)
}

impl ObjectFlags {
    pub fn is_visible(&self) -> bool {
        !self.hidden || self.revealed
    }
}


/// Resolved exit that takes into account the zone and the player's overlays
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedExit {
    pub direction: Direction,       // Direction of the exit
    pub from_room_id: RoomId,       // From Room ID
    pub from_room_key: String,
    pub to_room_id: RoomId,         // To Room ID
    pub to_room_key: String,
    pub flags: ExitFlags,
}

/// Resolved object that takes into account the zone and the player's overlays
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedObject {
    pub id: ObjectId,
    pub key: String,
    pub name: String,
    pub short: String,
    pub description: String,
    pub examine: Option<String>,
    pub nouns: Vec<String>,
    pub use_lua: Option<String>,
    pub position: Option<i32>,

    pub kv: KvResolved,
    pub flags: ObjectFlags,

    pub is_coin: bool,
    pub qty: i32,
}



#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---------- test helpers (local to tests) ----------
    // Helper: make a RoomId/ObjectId with a stable UUID for deterministic snapshots
    fn rid() -> RoomId { RoomId(Uuid::from_u128(1)) }
    fn rid_b() -> RoomId { RoomId(Uuid::from_u128(2)) }
    fn oid() -> ObjectId { ObjectId(Uuid::from_u128(3)) }

    // Helper: construct a minimal BlueprintRoom (used by build_room_view tests)
    fn mk_room() -> BlueprintRoom {
        BlueprintRoom {
            id: rid(),
            bp_id: BlueprintId(Uuid::from_u128(999)),
            key: "entry_hall".into(),
            title: "Entry Hall".into(),
            body: "A brushed-steel corridor hums with power.".into(),
            lockdown: false,
            short: Some("The station’s entry hall.".into()),
            hints: vec![],
        }
    }

    // Helper: construct a northbound exit
    fn mk_exit_north(to: RoomId, visible_when_locked: bool, default_locked: bool) -> BlueprintExit {
        BlueprintExit {
            from_room_id: rid(),
            from_room_key: "entry_hall".into(),
            dir: Direction::North,
            to_room_id: to,
            to_room_key: "blast_door".into(),
            description: Some("A heavy blast door to the north.".into()),
            visible_when_locked,
            default_locked,
        }
    }

    // Helper: construct a simple object
    fn mk_object_wrench() -> BlueprintObject {
        BlueprintObject {
            id: oid(),
            name: "wrench".into(),
            short: "A sturdy wrench.".into(),
            description: "A titanium-alloy wrench with knurled grip.".into(),
            examine: Some("It’s scuffed but reliable.".into()),
            on_use_lua: None,
            position: Some(10),
            nouns: vec!["tool".into(), "spanner".into()],
            object_kv: Kv { inner: HashMap::new() },
            initial_qty: Some(1),
            default_locked: false,
            default_revealed: false,
            takeable: true,
            stackable: false,
            is_coin: false,
        }
    }

    // Helper: Kv builder from (&str, &str) pairs
    fn kv(pairs: &[(&str, &str)]) -> Kv {
        Kv { inner: pairs.iter().map(|(k,v)| (k.to_string(), v.to_string())).collect() }
    }

    // ---------- normalize_when() ----------
    #[test]
    fn normalize_when_variants() {
        // From: normalize_when()
        assert_eq!(normalize_when("first look"), "first_look");
        assert_eq!(normalize_when("FIRST-LOOK"), "first_look");
        assert_eq!(normalize_when(" enter "), "enter");
        assert_eq!(normalize_when("after_fail"), "after_fail");
        // Unknowns default to "manual"
        assert_eq!(normalize_when("whenever"), "manual");
        assert_eq!(normalize_when(""), "manual");
    }

    // ---------- parse_hints_value() ----------
    #[test]
    fn parse_hints_legacy_array_of_strings() {
        // From: parse_hints_value()
        let v = Some(json!(["Use the console", "Search under the grate"]));
        let hints = parse_hints_value(v).expect("parse ok");
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].text, "Use the console");
        assert_eq!(hints[0].when, "manual"); // legacy defaults to manual
    }

    #[test]
    fn parse_hints_v2_objects_and_normalization() {
        // From: parse_hints_value()
        let v = Some(json!([
            {"text":"Check the panel","when":"first look","once":true},
            {"text":"Try north","when":"ENTER"},
            {"text":"Unknown mode","when":"freebie"}
        ]));
        let hints = parse_hints_value(v).expect("parse ok");
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].when, "first_look");
        assert_eq!(hints[0].once, Some(true));
        assert_eq!(hints[1].when, "enter");
        // unknown normalized to manual
        assert_eq!(hints[2].when, "manual");
    }

    #[test]
    fn parse_hints_single_object_and_null() {
        // From: parse_hints_value()
        let one = Some(json!({"text":"Just once","when":""}));
        let hints_one = parse_hints_value(one).expect("ok");
        assert_eq!(hints_one.len(), 1);
        assert_eq!(hints_one[0].text, "Just once");
        assert_eq!(hints_one[0].when, "manual"); // empty -> manual

        let none = Some(Value::Null);
        let hints_none = parse_hints_value(none).expect("ok");
        assert!(hints_none.is_empty());
    }

    // ---------- Kv::from / Kv::get ----------
    #[test]
    fn kv_from_only_keeps_strings() {
        // From: Kv::from()
        let v = json!({
            "a": "1",
            "b": 2,
            "c": true,
            "d": { "e": "nested" },
            "e": "ok"
        });
        let kv = Kv::from(v);
        assert_eq!(kv.get("a"), Some(&"1".to_string()));
        assert_eq!(kv.get("b"), Some(&"2".to_string()));
        assert_eq!(kv.get("c"), Some(&"true".to_string()));
        assert!(kv.get("d").is_none());
        assert_eq!(kv.get("e"), Some(&"ok".to_string()));
    }

    // ---------- ExitFlags::is_visible() ----------
    #[test]
    fn exit_flags_is_visible_matrix() {
        // From: ExitFlags::is_visible()
        let mut f = ExitFlags { locked: false, hidden: false, visible_when_locked: false };
        assert!(f.is_visible(), "unlocked + not hidden => visible");

        f.locked = true; f.hidden = false; f.visible_when_locked = false;
        assert!(!f.is_visible(), "locked + not visible when locked => hidden");

        f.locked = true; f.hidden = false; f.visible_when_locked = true;
        assert!(f.is_visible(), "locked + visible_when_locked => visible");

        f.locked = false; f.hidden = true; f.visible_when_locked = true;
        assert!(!f.is_visible(), "hidden overrides visibility");
    }

    // ---------- ObjectFlags::is_visible() ----------
    #[test]
    fn object_flags_is_visible_matrix() {
        // From: ObjectFlags::is_visible()
        let mut f = ObjectFlags { locked: false, hidden: false, revealed: false, takeable: true, stackable: false };
        assert!(f.is_visible(), "not hidden => visible");

        f.hidden = true; f.revealed = false;
        assert!(!f.is_visible(), "hidden && not revealed => not visible");

        f.hidden = true; f.revealed = true;
        assert!(f.is_visible(), "revealed pierces hidden");
    }

    // ---------- RoomView::object_by_noun() ----------
    #[test]
    fn object_by_noun_matches_name_and_synonyms_case_insensitive() {
        let room = mk_room();
        let objs = vec![mk_object_wrench()];
        let view = build_room_view_impl(
            &room,
            &[],               // bp_exits
            &objs,             // bp_objs
            &Kv::default(),    // bp_room_kv
            &Kv::default(),    // zone_room_kv
            &HashMap::new(),   // zone_obj_kv
            &HashMap::new(),   // zone_qty_override
            &Kv::default(),    // user_room_kv
            &HashMap::new(),   // user_obj_kv
            &HashMap::new(),   // user_qty_override
        );

        assert!(view.object_by_noun("wrench").is_some());
        assert!(view.object_by_noun("SpAnNeR").is_some(), "matches synonyms case-insensitive");
        assert!(view.object_by_noun("tool").is_some());
        assert!(view.object_by_noun("computer").is_none());
    }

    // ---------- build_room_view(): exit overlays precedence ----------
    #[test]
    fn build_room_view_exit_overlay_precedence() {
        // From: build_room_view()
        let room = mk_room();
        let exits = vec![
            // default: locked, but visible_when_locked = true
            mk_exit_north(rid_b(), /*visible_when_locked*/ true, /*default_locked*/ true),
        ];

        // No overrides: remains locked, but visible (due to visible_when_locked)
        let view = build_room_view_impl(
            &room, &exits, &[], &Kv::default(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
        );
        let north = &view.exits[0];
        assert!(north.flags.locked);
        assert!(north.flags.is_visible(), "visible_when_locked keeps it visible");

        // Zone override unlocks it: exit.north.locked=false
        let zone_kv = kv(&[("exit.north.locked", "false")]);
        let view = build_room_view_impl(
            &room, &exits, &[], &Kv::default(),
            &zone_kv, &HashMap::new(), &HashMap::new(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
        );
        let north = &view.exits[0];
        assert!(!north.flags.locked);

        // User override wins over zone, locks it again: exit.north.locked=true
        let user_kv = kv(&[("exit.north.locked", "true")]);
        let view = build_room_view_impl(
            &room, &exits, &[], &Kv::default(),
            &zone_kv, &HashMap::new(), &HashMap::new(),
            &user_kv, &HashMap::new(), &HashMap::new(),
        );
        let north = &view.exits[0];
        assert!(north.flags.locked);

        // Explicit visibility override: exit.north.visible=false hides even if visible_when_locked
        let user_kv = kv(&[("exit.north.visible", "false")]);
        let view = build_room_view_impl(
            &room, &exits, &[], &Kv::default(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
            &user_kv, &HashMap::new(), &HashMap::new(),
        );
        let north = &view.exits[0];
        assert!(!north.flags.is_visible(), "explicit visible=false hides it");
    }

    // ---------- build_room_view(): qty precedence (default -> zone -> user) ----------
    #[test]
    fn build_room_view_quantity_precedence() {
        // From: build_room_view()
        let room = mk_room();
        // Make stackable/coin just to emphasize qty behavior; initial_qty = 5
        let mut wrench = mk_object_wrench();
        wrench.stackable = true;
        wrench.is_coin = true;
        wrench.initial_qty = Some(5);

        let bp_objs = vec![wrench];
        let key = "wrench".to_string();

        // No overrides -> stays 5
        let view = build_room_view_impl(
            &room, &[], &bp_objs, &Kv::default(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
        );
        assert_eq!(view.objects[0].qty, 5);

        // Zone override -> 12
        let mut zone_qty = HashMap::new();
        zone_qty.insert(key.clone(), 12);
        let view = build_room_view_impl(
            &room, &[], &bp_objs, &Kv::default(),
            &Kv::default(), &HashMap::new(), &zone_qty,
            &Kv::default(), &HashMap::new(), &HashMap::new(),
        );
        assert_eq!(view.objects[0].qty, 12);

        // User override wins -> 99
        let mut user_qty = HashMap::new();
        user_qty.insert(key.clone(), 99);
        let view = build_room_view_impl(
            &room, &[], &bp_objs, &Kv::default(),
            &Kv::default(), &HashMap::new(), &zone_qty,
            &Kv::default(), &HashMap::new(), &user_qty,
        );
        assert_eq!(view.objects[0].qty, 99);
    }

    // ---------- build_room_view(): objects visibility flags vs revealed ----------
    #[test]
    fn build_room_view_object_visibility_and_revealed() {
        // From: build_room_view() + ObjectFlags::is_visible()
        let room = mk_room();
        let mut obj = mk_object_wrench();
        obj.default_revealed = false;
        let bp_objs = vec![obj];

        // No reveal, compute_object_visible may hide it -> object should be visible iff flags allow.
        let view = build_room_view_impl(
            &room, &[], &bp_objs, &Kv::default(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
        );
        let f = view.objects[0].flags;
        // We can’t assert exact hidden/visible of compute_object_visible(), but we can assert consistency:
        if f.hidden {
            assert!(!f.is_visible(), "hidden && !revealed => not visible");
        } else {
            assert!(f.is_visible(), "not hidden => visible");
        }

        // Mark revealed via user overlay; even if hidden, revealed should make it visible
        let user_obj_kv: HashMap<String, Kv> = HashMap::new(); // kv input to compute may vary
        let user_room_kv = kv(&[("revealed", "true")]); // resolves revealed flag
        let view = build_room_view_impl(
            &room, &[], &bp_objs, &Kv::default(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
            &user_room_kv, &user_obj_kv, &HashMap::new(),
        );
        let f = view.objects[0].flags;
        assert!(f.revealed, "revealed overlay applied");
        assert!(f.is_visible(), "revealed pierces hidden");
    }

    // ---------- build_room_view(): exits_by_dir index sanity ----------
    #[test]
    fn exits_by_dir_indexes_match_vector_positions() {
        // From: build_room_view()
        let room = mk_room();
        let exits = vec![
            mk_exit_north(rid_b(), true, false),
            BlueprintExit {
                from_room_id: rid(),
                from_room_key: "entry_hall".into(),
                dir: Direction::East,
                to_room_id: rid_b(),
                to_room_key: "side_room".into(),
                description: None,
                visible_when_locked: false,
                default_locked: false,
            }
        ];
        let view = build_room_view_impl(
            &room, &exits, &[], &Kv::default(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
            &Kv::default(), &HashMap::new(), &HashMap::new(),
        );
        assert_eq!(view.exits_by_dir.get(&Direction::North).copied(), Some(0));
        assert_eq!(view.exits_by_dir.get(&Direction::East).copied(), Some(1));
    }
}
