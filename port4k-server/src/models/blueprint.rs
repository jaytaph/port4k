use crate::db::DbResult;
use crate::db::error::DbError;
use crate::models::types::{AccountId, BlueprintId, RoomId};
use tokio_postgres::Row;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlueprintStatus {
    Draft,
    Published,
    Archived,
}

impl BlueprintStatus {
    fn parse(s: &str) -> DbResult<Self> {
        match s {
            "live" => Ok(BlueprintStatus::Published), // legacy support
            "draft" => Ok(BlueprintStatus::Draft),
            "published" => Ok(BlueprintStatus::Published),
            "archived" => Ok(BlueprintStatus::Archived),
            _ => Err(DbError::Decode("invalid blueprint.status".into())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Blueprint {
    pub id: BlueprintId,
    pub key: String,
    pub title: String,
    pub owner_id: AccountId,
    pub status: BlueprintStatus,
    pub entry_room_id: RoomId,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Blueprint {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let status_str: &str = row.try_get("status")?;
        let status = BlueprintStatus::parse(status_str)?;

        Ok(Self {
            id: row.try_get::<_, BlueprintId>("id")?,
            key: row.try_get("key")?,
            title: row.try_get("title")?,
            owner_id: row.try_get::<_, AccountId>("owner_id")?,
            status,
            entry_room_id: row.try_get::<_, RoomId>("entry_room_id")?,
            created_at: row.try_get("created_at")?,
        })
    }
}
