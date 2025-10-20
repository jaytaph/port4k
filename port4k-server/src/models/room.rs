use crate::db::DbResult;
use crate::db::error::DbError;
use crate::models::types::{BlueprintId, Direction, ObjectId, RoomId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio_postgres::Row;
use uuid::Uuid;
use crate::models::room_helpers::{compute_object_visible, merge_kv, resolve_bool, resolve_qty};
// static OBJ_REF_RE: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"\{obj:([a-zA-Z0-9_\- ]+)}").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hint {
    pub once: Option<bool>, // default: false
    pub text: String,
    pub when: String,          // first_look, enter, search, after_fail
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
pub struct BlueprintExit {
    pub from_room_id: RoomId,
    pub dir: Direction,
    pub to_room_id: RoomId,
    pub description: Option<String>,
    pub visible_when_locked: bool,
    pub default_locked: bool,
}

impl BlueprintExit {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let dir_s: String = row.try_get("dir")?;
        let dir = Direction::parse(&dir_s)
            .ok_or_else(|| DbError::Decode(format!("invalid direction in bp_exits: {}", dir_s)))?;

        Ok(Self {
            from_room_id: row.try_get("from_room_id")?,
            dir,
            to_room_id: row.try_get("to_room_id")?,
            description: row.try_get("description")?,
            visible_when_locked: row.try_get("visible_when_locked")?,
            default_locked: row.try_get("locked")?,
        })
    }
}

/// Row model for `bp_objects`.
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
    pub discovery: Discovery,

    pub object_kv: Kv,
    pub initial_qty: Option<i32>,
    pub default_locked: bool,
    pub default_revealed: bool,
    pub takeable: bool,
    pub stackable: bool,
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
            discovery: Discovery::default(),

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoomScripts {
    pub on_enter_lua: Option<String>,
    pub on_command_lua: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StrOrVec {
    Str(String),
    Vec(Vec<String>),
}



// pub fn rows_to_room_kv(rows: Vec<Row>) -> AppResult<Kv> {
//     let mut kv = Kv::new();
//     for r in rows {
//         let key: String = r.get(0);                // SELECT key, value FROM ...
//         let value: Value = r.get(1);               // jsonb column
//         let list = json_to_vec_strings(value)?;    // enforce type
//         kv.insert(key, StrOrVec::Vec(list));
//     }
//     Ok(kv)
// }

pub type KvResolved = HashMap<String, Vec<String>>;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Kv {
    pub inner: HashMap<String, StrOrVec>,
}

impl Kv {
    pub fn from(data: serde_json::Value) -> Kv {
        let mut kv = Kv::default();
        if let serde_json::Value::Object(map) = data {
            for (k, v) in map {
                match v {
                    serde_json::Value::String(s) => {
                        kv.inner.insert(k, StrOrVec::Str(s));
                    }
                    serde_json::Value::Array(arr) => {
                        let list: Vec<String> = arr
                            .into_iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect();
                        kv.inner.insert(k, StrOrVec::Vec(list));
                    }
                    _ => {
                        // Ignore other types
                    }
                }
            }
        }
        kv
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum Discovery {
    #[default]
    Visible, // always listed
    Hidden, // never listed until discovered
    Obscured { dc: u8 }, // requires a perception check >= dc
    Conditional { key: String, value: String }, // visible if room_kv[key]==value
    Scripted, // let Lua decide
}

// /// Runtime-friendly object with resolved nouns.
// #[derive(Debug, Clone, Serialize, Deserialize, Default)]
// pub struct RoomObject {
//     /// Blueprint Object ID
//     pub id: ObjectId,
//     /// Name of the object (ie: "wrench")
//     pub name: String,
//     /// Short description (one-liner)
//     pub short: String,
//     /// Full description
//     pub description: String,
//     /// Examine text (if any)
//     pub examine: Option<String>,
//     /// Lua script to run when `use`
//     pub use_lua: Option<String>,
//     /// Position for ordering (optional)
//     pub position: Option<i32>,
//     /// Synonyms / alternate nouns (terminal, console, computer, screen)
//     pub nouns: Vec<String>,
//     /// How is this object discovered in the room?
//     pub discovery: Discovery,
//
//     pub object_kv: Kv,
//
//     // Overlay
//     pub initial_qty: Option<i32>,
//     pub qty: Option<i32>,
//     pub locked: bool,
//     pub revealed: bool,
//     pub takeable: bool,
//     pub stackable: bool,
//     pub is_coin: bool,
// }
//
// impl RoomObject {
//     fn has_flag(list: &[String], flag: &str) -> bool {
//         list.iter().any(|s| s.eq_ignore_ascii_case(flag))
//     }
//
//     pub fn is_visible(&self) -> bool {
//         // visible objects are either non-stackable items, or revealed stackables
//         !self.stackable || self.revealed
//     }
//
//     pub fn is_visible_to(&self, rv: &RoomView, zr: &ZoneRoomState) -> bool {
//         let discovered = is_visible_to(self, rv, zr);
//         discovered && self.is_visible()
//     }
//
//     pub fn from_rows(row_obj: &RoomObjectRow, kv: Kv, nouns: &[String]) -> Self {
//         Self {
//             id: row_obj.id,
//             name: row_obj.name.clone(),
//             short: row_obj.short.clone(),
//             description: row_obj.description.clone(),
//             examine: row_obj.examine.clone(),
//             use_lua: row_obj.on_use_lua.clone(),
//             position: row_obj.position,
//             object_kv: kv,
//             // state: row_obj.state.iter().map(|s| (s.clone(), "true".to_string())).collect(),
//             nouns: nouns.as_ref().to_vec(),
//             discovery: Discovery::Visible,
//
//             initial_qty: None,
//             qty: None,
//             locked: false,
//             revealed: false,
//             takeable: false,
//             stackable: false,
//             is_coin: false,
//         }
//     }
// }

/// Runtime view the engine uses.
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
    // /// from fn: `RoomView::visible_exits`
    // pub fn visible_exits(&self) -> impl Iterator<Item = &RoomExitRow> {
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

    // /// from fn: `RoomView::render_body_with_object_refs`
    // /// Replaces `{obj:name}` with the object's `short` text.
    // pub fn render_body_with_object_refs(&self) -> String {
    //     OBJ_REF_RE
    //         .replace_all(&self.room.body, |caps: &regex::Captures| {
    //             let key = &caps[1];
    //             self.object_by_noun(key)
    //                 .map(|o| o.short.as_str())
    //                 .unwrap_or(key)
    //                 .to_string()
    //         })
    //         .into_owned()
    // }
    //
    // pub fn with_overlay(mut self, overlay: &[ZoneObjectState]) -> Self {
    //     let by_id: HashMap<ObjectId, &ZoneObjectState> = overlay.iter().map(|z| (z.obj_id, z)).collect();
    //
    //     for o in &mut self.objects {
    //         if let Some(z) = by_id.get(&o.id) {
    //             // qty
    //             if z.qty.is_some() {
    //                 o.qty = z.qty;
    //             }
    //
    //             o.locked = o.locked || RoomObject::has_flag(&z.flags, "locked");
    //             o.revealed = o.revealed || RoomObject::has_flag(&z.flags, "revealed");
    //         } else {
    //             o.qty = o.initial_qty;
    //         }
    //     }
    //
    //     self.objects.retain(|o| !o.stackable || o.qty.unwrap_or(0) > 0);
    //
    //     self
    // }
    //
    // pub fn visible_objects<'a>(&'a self, zr: &'a ZoneRoomState) -> impl Iterator<Item = &'a RoomObject> + 'a {
    //     self.objects.iter().filter(|o| is_visible_to(o, self, zr))
    // }
}

#[derive(Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ExitFlags {
    /// Is this exit currently locked (e.g. door closed)?
    pub locked: Option<bool>,
    /// Is it temporarily hidden (e.g. disguised, secret panel closed)?
    pub hidden: Option<bool>,
}

#[derive(Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ObjectFlags {
    /// Player/zone override for "locked" state.
    pub locked: Option<bool>,
    /// Player/zone override for "revealed" state.
    pub revealed: Option<bool>,
    /// Player/zone override for "takeable" flag.
    pub takeable: Option<bool>,
    /// Player/zone override for "hidden" state (like invisible).
    pub hidden: Option<bool>,
}

/// Builds up a complete room view by assembling blueprint, zone, and user data.
fn build_room_view(
    bp_room: &BlueprintRoom,
    bp_exits: &[BlueprintExit],
    bp_objs: &[BlueprintObject],

    zone_room_kv: &Kv,
    user_room_kv: &Kv,
    bp_room_kv: &Kv,

    zone_exit_kv: &HashMap<Direction, Kv>,
    user_exit_kv: &HashMap<Direction, Kv>,

    zone_obj_kv: &HashMap<String, Kv>,
    user_obj_kv: &HashMap<String, Kv>,

    // zone_exit_flags: &HashMap<Direction, ExitFlags>,
    // user_exit_flags: &HashMap<Direction, ExitFlags>,
    // zone_obj_flags: &HashMap<String, ObjectFlags>,
    // user_obj_flags: &HashMap<String, ObjectFlags>,

    zone_qty_override: &HashMap<String, i32>,
    user_qty_override: &HashMap<String, i32>,
) -> RoomView {
    let room_kv = merge_kv(bp_room_kv, Some(zone_room_kv), Some(user_room_kv));

    let mut exits = Vec::new();
    let mut exits_by_dir = HashMap::new();
    for e in bp_exits {
        let kv = merge_kv_exit(e.dir, None, zone_exit_kv.get(&e.dir), user_exit_kv.get(&e.dir));
        let locked = resolve_bool(
            user_exit_flags.get(&e.dir).map(|f| f.locked),
            zone_exit_flags.get(&e.dir).map(|f| f.locked),
            Some(e.default_locked),
        );

        let visible = resolve_exit_visible(e.visible_when_locked, locked);
        let idx = exits.len();
        exits.push(ResolvedExit {
            direction: e.dir,
            from_room_id: e.from_room_id,
            to_room_id: e.to_room_id,
            kv,
            locked,
            visible,
        });
        exits_by_dir.insert(e.dir, idx);
    }


    let mut objects = Vec::new();
    let mut objects_by_key = HashMap::new();
    for o in bp_objs {
        let key = o.name.clone();
        let kv = merge_kv(&o.object_kv, zone_obj_kv.get(&key).unwrap_or(&Kv::default()), user_obj_kv.get(&key).unwrap_or(&Kv::default()));
        let qty = resolve_qty(
            user_qty_override.get(&key).copied(),
            zone_qty_override.get(&key).copied(),
            o.initial_qty.unwrap_or(1),
        );
        let locked = resolve_bool(
            user_obj_flags.get(&key).map(|f| f.locked),
            zone_obj_flags.get(&key).map(|f| f.locked),
            Some(o.default_locked),
        );
        let revealed = resolve_bool(
            user_obj_flags.get(&key).map(|f| f.revealed),
            zone_obj_flags.get(&key).map(|f| f.revealed),
            Some(o.default_revealed),
        );
        let visible = compute_object_visible(&kv, revealed);

        // let idx = objects.len();
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
            locked,
            revealed,
            visible,
            takeable: o.takeable,
            stackable: o.stackable,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedExit {
    pub direction: Direction,
    pub from_room_id: RoomId,
    pub to_room_id: RoomId,
    pub kv: KvResolved,
    pub locked: bool,
    pub visible: bool,
}

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
    pub qty: i32,
    pub locked: bool,
    pub revealed: bool,
    pub visible: bool,
    pub takeable: bool,
    pub stackable: bool,
    pub is_coin: bool,
}
