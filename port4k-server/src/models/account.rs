use tokio_postgres::Row;
use crate::db::DbResult;
use crate::db::error::DbError;
use crate::error::{AppResult, DomainError};
use crate::models::types::{AccountId, RoomId, ZoneId};

#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,
    pub zone_id: Option<ZoneId>,
    pub current_room_id: Option<RoomId>,
    pub xp: u32,
    pub health: u32,
    pub coins: u32,
    pub inventory: Vec<String>,
    pub flags: Vec<String>,
}

impl Account {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let xp_i: i32     = row.try_get("xp")?;
        let health_i: i32 = row.try_get("health")?;
        let coins_i: i32  = row.try_get("coins")?;

        let xp     = u32::try_from(xp_i).map_err(|_| DbError::Decode("xp < 0".into()))?;
        let health = u32::try_from(health_i).map_err(|_| DbError::Decode("health < 0".into()))?;
        let coins  = u32::try_from(coins_i).map_err(|_| DbError::Decode("coins < 0".into()))?;

        // Prefer decoding JSON directly if schema is jsonb array of text
        let inventory: Option<Vec<String>> = row.try_get("inventory").ok();
        let flags:     Option<Vec<String>> = row.try_get("flags").ok();

        Ok(Self {
            id: row.try_get::<_, AccountId>("id")?,
            username: row.try_get("username")?,
            email: row.try_get("email")?,
            password_hash: row.try_get("password_hash")?,
            role: row.try_get("role")?,
            created_at: row.try_get("created_at")?,
            last_login: row.try_get("last_login")?,
            zone_id: row.try_get::<_, Option<ZoneId>>("zone_id")?,
            current_room_id: row.try_get::<_, Option<RoomId>>("current_room_id")?,
            xp, health, coins,
            inventory: inventory.unwrap_or_default(),
            flags: flags.unwrap_or_default(),
        })
    }

    pub fn validate_username(s: &str) -> AppResult<()> {
        let s = s.trim();
        if s.is_empty() {
            return Err(DomainError::Validation { field: "username", message: "cannot be empty".into() });
        }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' )) {
            return Err(DomainError::Validation {
                field: "username",
                message: "only alphanumeric, hyphen, underscore allowed".into(),
            });
        }
        Ok(())
    }
}