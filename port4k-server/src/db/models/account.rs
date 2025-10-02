use anyhow::anyhow;
use tokio_postgres::Row;
use crate::db::json_string_vec;
use crate::db::types::{AccountId, RoomId, ZoneId};

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
    pub fn from_row(row: Row) -> Self {
        let xp: i64 = row.get("xp");
        let health: i64 = row.get("health");
        let coins: i64 = row.get("coins");

        let inv_json: Option<serde_json::Value> = row.try_get("inventory").ok();
        let flags_json: Option<serde_json::Value> = row.try_get("flags").ok();

        Account {
            id: row.get::<_, AccountId>("id"),
            username: row.get("username"),
            role: row.get("role"),
            password_hash: row.get("password_hash"),
            created_at: row.get("created_at"),
            last_login: row.get("last_login"),
            zone_id: row.get::<_, Option<ZoneId>>("zone_id"),
            current_room_id: row.get::<_, Option<RoomId>>("current_room_id"),
            xp: xp as u64,
            health: health as u64,
            coins: coins as u64,
            inventory: json_string_vec(inv_json),
            flags: json_string_vec(flags_json),
        }
    }

    pub fn validate_username(s: &str) -> anyhow::Result<()> {
        let s = s.trim();
        if s.is_empty() { return Err(anyhow!("Username cannot be empty.")) }

        if !s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' )) {
            return Err(anyhow!("Username can only contain alphanumeric characters, hyphens, and underscores."));
        }

        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("username already exists")]
    UsernameTaken,
    #[error("invalid username or password")]
    InvalidCredentials,
    #[error("password policy not met")]
    WeakPassword,
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}