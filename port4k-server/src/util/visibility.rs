use crate::models::room::{Discovery, RoomObject, RoomView, ZoneRoomState};

pub fn is_visible_to(obj: &RoomObject, rv: &RoomView, zr: &ZoneRoomState) -> bool {
    if zr.discovered_objects.contains(&obj.id) {
        return true;
    }
    match &obj.discovery {
        Discovery::Visible => true,
        Discovery::Hidden => false,
        Discovery::Obscured { .. } => false, // until discovered via checks
        Discovery::Conditional { key, value } => rv
            .room_kv
            .get(key)
            .map(|vals| vals.iter().any(|v| v == value))
            .unwrap_or(false),
        Discovery::Scripted => false, // let Lua flip discovery when ready
    }
}
