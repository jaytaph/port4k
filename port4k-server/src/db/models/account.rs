use crate::db::types::{AccountId, RoomId, ZoneId};

#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub role: String,
    pub created_at: chrono::NaiveDateTime,
    pub last_login: chrono::NaiveDateTime,
    pub zone_id: ZoneId,
    pub current_room_id: RoomId,
    pub xp: u64,
    pub health: u64,
    pub coins: u64,
    pub inventory: Vec<String>,
    pub flags: Vec<String>,
}