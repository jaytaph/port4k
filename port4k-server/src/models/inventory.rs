use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,

    #[serde(default)]
    pub nouns: Vec<String>,

    #[serde(default)]
    pub short: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub examine: Option<String>,

    #[serde(default)]
    pub stackable: bool,

    #[serde(default)]
    pub weight: f32, // Optional: for inventory limits

    #[serde(default)]
    pub consumable: bool, // Optional: can be consumed on use

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>, // For custom properties
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemInstance {
    pub item_id: String, // Reference to Item definition
    pub quantity: i32,   // For stackable items
    pub location: ItemLocation,

    #[serde(default)]
    pub state: HashMap<String, serde_json::Value>, // Instance-specific state
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemLocation {
    Room {
        zone_id: String,
        room_id: String,
    },
    Object {
        zone_id: String,
        room_id: String,
        object_id: String,
    }, // Inside toolkit
    Player {
        user_id: String,
    },
    Dropped {
        zone_id: String,
        room_id: String,
    }, // On floor
}
