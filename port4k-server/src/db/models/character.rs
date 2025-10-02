use tokio_postgres::Row;
use crate::db::json_string_vec;
use crate::db::types::{AccountId, CharacterId, RoomId, ZoneId};

#[derive(Debug, Clone)]
pub struct Character {
    pub id: CharacterId,
    pub account_id: AccountId,
    pub name: String,
    pub zone_id: ZoneId,
    pub room_id: RoomId,
    pub stats: Vec<String>,
    pub created_at: chrono::NaiveDateTime,
}

impl Character {
    pub fn from_row(row: Row) -> Self {
        let stats_json: Option<serde_json::Value> = row.try_get("stats").ok();

        Character {
            id: row.get::<_, CharacterId>("id"),
            account_id: row.get::<_, AccountId>("account_id"),
            name: row.get("name"),
            zone_id: row.get::<_, ZoneId>("zone_id"),
            room_id: row.get::<_, RoomId>("room_id"),
            stats: json_string_vec(stats_json),
            created_at: row.get("created_at"),
        }
    }
}