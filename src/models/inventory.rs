use crate::models::types::{AccountId, BlueprintId, ItemId, ObjectId, RoomId, RealmId};

#[derive(Debug, Clone)]
pub struct Item {
    /// Catalog ID (from bp_items_catalog.id)
    pub id: ItemId,

    /// Blueprint ID this item belongs to
    pub bp_id: BlueprintId,

    /// Unique key within blueprint (e.g., "multi_spanner")
    pub item_key: String,

    /// Display name (e.g., "Multi-Spanner")
    pub name: String,

    /// Searchable nouns (e.g., ["spanner", "tool", "wrench"])
    pub nouns: Vec<String>,

    /// Short description (e.g., "versatile multi-spanner")
    pub short: String,

    /// Full description
    pub description: String,

    /// Optional detailed examination text
    pub examine: Option<String>,

    /// Whether multiple instances can stack
    pub stackable: bool,
}

/// Item instance in the game world
/// This is an actual item that exists in a realm with a location
#[derive(Debug, Clone)]
pub struct ItemInstance {
    /// Instance ID (from items.id)
    pub instance_id: ItemId,

    /// Realm this instance belongs to
    pub realm_id: RealmId,

    /// Reference to catalog definition
    pub catalog_id: ItemId,

    /// Location of this instance
    pub location: ItemLocation,

    /// Stack size (1 for non-stackable items)
    pub quantity: i32,

    /// Custom instance state (durability, charges, etc.)
    pub condition: Option<serde_json::Value>,

    // Denormalized fields from catalog (for convenience)
    pub item_key: String,
    pub name: String,
    pub short: String,
    pub description: String,
    pub examine: Option<String>,
    pub stackable: bool,
    pub nouns: Vec<String>,

    /// Timestamps
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl ItemInstance {
    /// Check if this item is in a specific location
    pub fn is_at(&self, location: ItemLocation) -> bool {
        self.location == location
    }

    /// Check if owned by player
    pub fn is_owned_by(&self, account_id: AccountId) -> bool {
        self.location.is_in_player_inventory(account_id)
    }

    /// Check if in a room
    pub fn is_in_room(&self, room_id: RoomId) -> bool {
        self.location.is_in_room(room_id)
    }

    /// Get display text for inventory listing
    pub fn display_text(&self) -> String {
        if self.stackable && self.quantity > 1 {
            format!("{} (x{})", self.short, self.quantity)
        } else {
            self.short.clone()
        }
    }
}

/// Represents where an item instance is located in the game world
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemLocation {
    /// Item is on the ground in a room
    Room(RoomId),
    /// Item is in a player's inventory
    Player(AccountId),
    /// Item is inside an object/container
    Object(ObjectId),
    /// Item is inside another item (nested container)
    Container(ItemId),
}

impl ItemLocation {
    /// Convert location to database column values
    /// Returns (room_id, account_id, object_id, container_item_id)
    pub fn to_db_columns(&self) -> (Option<RoomId>, Option<AccountId>, Option<ObjectId>, Option<ItemId>) {
        match self {
            ItemLocation::Room(id) => (Some(*id), None, None, None),
            ItemLocation::Player(id) => (None, Some(*id), None, None),
            ItemLocation::Object(id) => (None, None, Some(*id), None),
            ItemLocation::Container(id) => (None, None, None, Some(*id)),
        }
    }

    /// Create from database column values
    pub fn from_db_columns(
        room_id: Option<RoomId>,
        account_id: Option<AccountId>,
        object_id: Option<ObjectId>,
        container_item_id: Option<ItemId>,
    ) -> Result<Self, String> {
        match (room_id, account_id, object_id, container_item_id) {
            (Some(id), None, None, None) => Ok(ItemLocation::Room(id)),
            (None, Some(id), None, None) => Ok(ItemLocation::Player(id)),
            (None, None, Some(id), None) => Ok(ItemLocation::Object(id)),
            (None, None, None, Some(id)) => Ok(ItemLocation::Container(id)),
            _ => Err("Invalid item location: exactly one location must be set".to_string()),
        }
    }

    /// Check if item is in player inventory
    pub fn is_in_player_inventory(&self, account_id: AccountId) -> bool {
        matches!(self, ItemLocation::Player(id) if *id == account_id)
    }

    /// Check if item is in a room
    pub fn is_in_room(&self, room_id: RoomId) -> bool {
        matches!(self, ItemLocation::Room(id) if *id == room_id)
    }
}
