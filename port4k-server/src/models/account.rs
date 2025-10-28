use crate::db::DbResult;
use crate::db::error::DbError;
use crate::error::{AppResult, DomainError};
use crate::models::inventory::{Inventory, InventoryItem};
use crate::models::types::{AccountId, RoomId, ZoneId};
use bytes;
use postgres_types::private::BytesMut;
use postgres_types::{FromSql, IsNull, ToSql, Type};
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio_postgres::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountRole {
    Admin,   // Can do everything
    Builder, // Can build new rooms / blueprints
    User,    // Regular user
}

impl ToSql for AccountRole {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<postgres_types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        let s = match self {
            AccountRole::Admin => "admin",
            AccountRole::Builder => "builder",
            AccountRole::User => "user",
        };
        s.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        ty == &Type::TEXT
    }

    fn to_sql_checked(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        self.to_sql(ty, out)
    }
}

impl FromSql<'_> for AccountRole {
    fn from_sql(ty: &postgres_types::Type, raw: &[u8]) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let s = String::from_sql(ty, raw)?;
        match s.as_str() {
            "admin" => Ok(AccountRole::Admin),
            "builder" => Ok(AccountRole::Builder),
            "user" => Ok(AccountRole::User),
            _ => Err(format!("Unknown account role: {}", s).into()),
        }
    }

    fn accepts(ty: &Type) -> bool {
        ty == &Type::TEXT
    }
}

impl std::fmt::Display for AccountRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountRole::Admin => write!(f, "admin"),
            AccountRole::Builder => write!(f, "builder"),
            AccountRole::User => write!(f, "user"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    /// Unique Account ID
    pub id: AccountId,
    /// Username (distinct)
    pub username: String,
    /// Email address registered to the account
    pub email: String,
    /// Hashed password (argon)
    pub password_hash: String,
    /// Role (e.g., "user", "admin")
    pub role: AccountRole,
    /// Account creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last login timestamp
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,
}

impl Account {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        // let xp_i: i32 = row.try_get("xp")?;
        // let health_i: i32 = row.try_get("health")?;
        // let coins_i: i32 = row.try_get("coins")?;

        // let xp = u32::try_from(xp_i).map_err(|_| DbError::Decode("xp < 0".into()))?;
        // let health = u32::try_from(health_i).map_err(|_| DbError::Decode("health < 0".into()))?;
        // let coins = u32::try_from(coins_i).map_err(|_| DbError::Decode("coins < 0".into()))?;

        // Prefer decoding JSON directly if schema is jsonb array of text
        // let inventory: Option<Vec<String>> = row.try_get("inventory").ok();
        // let flags: Option<Vec<String>> = row.try_get("flags").ok();

        Ok(Self {
            id: row.try_get::<_, AccountId>("id")?,
            username: row.try_get("username")?,
            email: row.try_get("email")?,
            password_hash: row.try_get("password_hash")?,
            role: row.try_get("role")?,
            created_at: row.try_get("created_at")?,
            last_login: row.try_get("last_login")?,
            // zone_id: row.try_get::<_, Option<ZoneId>>("zone_id")?,
            // current_room_id: row.try_get::<_, Option<RoomId>>("current_room_id")?,
            // xp,
            // health,
            // coins,
            // inventory: inventory.unwrap_or_default(),
            // flags: flags.unwrap_or_default(),
        })
    }

    pub fn validate_username(s: &str) -> AppResult<()> {
        let s = s.trim();
        if s.is_empty() {
            return Err(DomainError::Validation {
                field: "username",
                message: "cannot be empty".into(),
            });
        }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
            return Err(DomainError::Validation {
                field: "username",
                message: "only alphanumeric, hyphen, underscore allowed".into(),
            });
        }
        Ok(())
    }
}

pub struct UserZoneData {
    /// Account ID of the user
    pub account_id: AccountId,
    /// Zone ID of the user
    pub zone_id: Option<ZoneId>,
    /// Current room ID of the user
    pub current_room_id: Option<RoomId>,
    /// Experience points
    pub xp: u32,
    /// Health points
    pub health: u32,
    /// In-game currency
    pub coins: u32,
    /// Inventory items
    pub inventory: Inventory,
}

impl UserZoneData {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        let xp_i: i32 = row.try_get("xp")?;
        let health_i: i32 = row.try_get("health")?;
        let coins_i: i32 = row.try_get("coins")?;

        let xp = u32::try_from(xp_i).map_err(|_| DbError::Decode("xp < 0".into()))?;
        let health = u32::try_from(health_i).map_err(|_| DbError::Decode("health < 0".into()))?;
        let coins = u32::try_from(coins_i).map_err(|_| DbError::Decode("coins < 0".into()))?;

        // Prefer decoding JSON directly if schema is jsonb array of text
        let inventory_json: Option<serde_json::Value> = row.try_get("inventory").ok();
        let mut items = Vec::new();
        if let Some(json) = inventory_json {
            if let Some(array) = json.as_array() {
                for item in array {
                    if let (Some(object_id), Some(quantity)) = (
                        item.get("object_id").and_then(|v| v.as_str()),
                        item.get("quantity").and_then(|v| v.as_u64()),
                    ) {
                        items.push(InventoryItem {
                            object_id: object_id.to_string(),
                            quantity: quantity as u32,
                        });
                    }
                }
            }
        }

        Ok(Self {
            account_id: row.try_get::<_, AccountId>("account_id")?,
            zone_id: row.try_get::<_, Option<ZoneId>>("zone_id")?,
            current_room_id: row.try_get::<_, Option<RoomId>>("current_room_id")?,
            xp,
            health,
            coins,
            inventory: Inventory {
                items,
                max_item_count: row
                    .try_get::<_, i32>("max_item_count")?
                    .try_into()
                    .map_err(|_| DbError::Decode("max_item_count < 0".into()))?,
            },
        })
    }
}
