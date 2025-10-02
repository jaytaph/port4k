use tokio_postgres::Row;
use crate::db::types::{AccountId, BlueprintId, RoomId};

#[derive(Clone, Debug)]
pub enum BlueprintStatus {
    Draft,
    Published,
    Archived,
}

#[derive(Debug, Clone)]
pub struct Blueprint {
    pub id: BlueprintId,
    pub key: String,
    pub title: String,
    pub owner: AccountId,
    pub status: BlueprintStatus,
    pub entry_room_id: RoomId,
    pub created_at: chrono::NaiveDateTime,
}

impl Blueprint {
    pub fn from_row(row: Row) -> Self {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "draft" => BlueprintStatus::Draft,
            "published" => BlueprintStatus::Published,
            "archived" => BlueprintStatus::Archived,
            _ => BlueprintStatus::Draft, // Default to Draft if unknown
        };

        Blueprint {
            id: row.get::<_, BlueprintId>("id"),
            key: row.get("key"),
            title: row.get("title"),
            owner: row.get::<_, AccountId>("owner"),
            status,
            entry_room_id: row.get::<_, RoomId>("entry_room_id"),
            created_at: row.get("created_at"),
        }
    }
}