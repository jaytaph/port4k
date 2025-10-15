use crate::models::room::{Discovery, RoomView, ZoneRoomState};
use crate::models::types::ObjectId;

pub fn passive_discovery(
    rv: &RoomView,
    zr: &mut ZoneRoomState,
    perception: u8, // from character sheet; fall back to default
) -> Vec<ObjectId> {
    let mut revealed = vec![];
    for obj in &rv.objects {
        if let Discovery::Obscured { dc } = obj.discovery
            && perception >= dc
                && zr.discovered_objects.insert(obj.id) {
                    revealed.push(obj.id);
                }
    }
    revealed
}
