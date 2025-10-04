use tokio_postgres::Row;
use crate::db::models::blueprint::Blueprint;
use crate::db::types::{AccountId, RoomId, ZoneId};

#[derive(Clone, Debug)]
pub enum ZoneKind {
    Live,
    Draft,
    Test { owner: AccountId },
}

#[derive(Clone, Debug)]
pub struct ZoneRef {
    pub id: ZoneId,
    pub kind: ZoneKind,
    pub blueprint: Blueprint,
}

pub struct WorldCursor {
    /// Zone in which we reside
    pub zone: ZoneRef,
    /// Room id in the current zone/world
    pub room_id: RoomId,
    /// id of the room in the previous zone/world
    pub prev_room_id: Option<RoomId>,
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub id: ZoneId,
    pub key: String,
    pub title: String,
    pub kind: ZoneKind,
    pub created_at: chrono::NaiveDateTime,
}

impl Zone {
    pub fn from_row(row: Row) -> Self {
        let kind_str: String = row.get("kind");
        let kind = match kind_str.as_str() {
            "live" => ZoneKind::Live,
            "draft" => ZoneKind::Draft,
            _ => ZoneKind::Draft, // Default to Draft if unknown
        };

        Zone {
            id: row.get::<_, ZoneId>("id"),
            key: row.get("key"),
            title: row.get("title"),
            kind,
            created_at: row.get("created_at"),
        }
    }
}