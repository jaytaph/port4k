mod account;
mod account_db;
mod inventory;
mod inventory_db;
mod realm;
mod realm_db;
mod room;
mod room_db;
mod user;
mod user_db;

pub use account_db::AccountRepository;
pub use inventory_db::InventoryRepository;
pub use realm_db::RealmRepository;
pub use room_db::RoomRepository;
pub use user_db::UserRepository;

pub use account::AccountRepo;
pub use inventory::InventoryRepo;
pub use realm::RealmRepo;
pub use room::RoomRepo;
pub use user::UserRepo;

/// Even though room_ids are globally unique, we still use a combination of
/// blueprint key and room key to identify a room sometimes. So we have a combined key
/// with the unique identifiers for both the blueprint and the room. This combination is also
/// globally unique.
/// The reason we do this is that when creating or editing a blueprint, the rooms
/// are identified by their blueprint key and room key, not by their room_id.
/// The room_id is only assigned when the room is created in the database.
/// This allows us to refer to rooms in a blueprint before they are created in the database.
#[derive(Debug, Clone)]
pub struct BlueprintAndRoomKey {
    pub bp_key: String,
    pub room_key: String,
}

impl BlueprintAndRoomKey {
    pub fn new(bp_key: &str, room_key: &str) -> Self {
        Self {
            bp_key: bp_key.to_string(),
            room_key: room_key.to_string(),
        }
    }
}
