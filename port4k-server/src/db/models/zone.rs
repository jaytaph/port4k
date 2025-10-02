use tokio_postgres::Row;
use crate::db::types::{ZoneId};

#[derive(Clone, Debug)]
pub enum ZoneKind {
    Live,
    Draft,
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