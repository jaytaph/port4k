use tokio_postgres::Row;
use crate::db::{DbError, DbResult};
use crate::error::{AppError, AppResult};
use crate::models::types::{AccountId, RoomId, ZoneId};

#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: chrono::NaiveDateTime,
    pub last_login: Option<chrono::NaiveDateTime>,
    pub zone_id: Option<ZoneId>,
    pub current_room_id: Option<RoomId>,
    pub xp: u64,
    pub health: u64,
    pub coins: u64,
    pub inventory: Vec<String>,
    pub flags: Vec<String>,
}

impl Account {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let xp_i: i64     = row.try_get("xp")?;
        let health_i: i64 = row.try_get("health")?;
        let coins_i: i64  = row.try_get("coins")?;

        let xp     = u64::try_from(xp_i).map_err(|_| DbError::Decode("xp < 0"))?;
        let health = u64::try_from(health_i).map_err(|_| DbError::Decode("health < 0"))?;
        let coins  = u64::try_from(coins_i).map_err(|_| DbError::Decode("coins < 0"))?;

        // Prefer decoding JSON directly if schema is jsonb array of text
        let inventory: Option<Vec<String>> = row.try_get("inventory").ok();
        let flags:     Option<Vec<String>> = row.try_get("flags").ok();

        Ok(Self {
            id: row.try_get::<_, AccountId>("id")?,
            username: row.try_get("username")?,
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
            return Err(AppError::Validation { field: "username", message: "cannot be empty".into() });
        }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' )) {
            return Err(AppError::Validation {
                field: "username",
                message: "only alphanumeric, hyphen, underscore allowed".into(),
            });
        }
        Ok(())
    }
}