use crate::db::DbResult;
use crate::db::error::DbError;
use crate::error::{AppResult, DomainError};
use crate::models::types::{AccountId, RealmId, RoomId};
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
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
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
    fn from_sql(ty: &Type, raw: &[u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
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
    /// Whether the account is locked due to too many failed login attempts
    pub locked_out: bool,
    /// Whether to show the message of the day on login
    pub show_motd: bool,

    /// realm/room where we currently are (if any)
    pub current_realm_id: Option<RealmId>,
    pub current_room_id: Option<RoomId>,
    /// realm/room where we spawn into when ded
    pub spawn_realm_id: Option<RealmId>,
    pub spawn_room_id: Option<RoomId>,

    // These settings are global per account and are inter-realm
    pub health: u32,
    pub xp: u32,
    pub coins: u32,
}

impl Account {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        Ok(Self {
            id: row.try_get::<_, AccountId>("id")?,
            username: row.try_get("username")?,
            email: row.try_get("email")?,
            password_hash: row.try_get("password_hash")?,
            role: row.try_get("role")?,
            created_at: row.try_get("created_at")?,
            last_login: row.try_get("last_login")?,
            locked_out: row.try_get("locked_out")?,
            show_motd: row.try_get("show_motd")?,
            current_realm_id: row.try_get::<_, Option<RealmId>>("current_realm_id")?,
            current_room_id: row.try_get::<_, Option<RoomId>>("current_room_id")?,
            spawn_realm_id: row.try_get::<_, Option<RealmId>>("spawn_realm_id")?,
            spawn_room_id: row.try_get::<_, Option<RoomId>>("spawn_room_id")?,
            health: row
                .try_get::<_, i32>("health")?
                .try_into()
                .map_err(|_| DbError::Decode("health < 0".into()))?,
            xp: row
                .try_get::<_, i32>("xp")?
                .try_into()
                .map_err(|_| DbError::Decode("xp < 0".into()))?,
            coins: row
                .try_get::<_, i32>("coins")?
                .try_into()
                .map_err(|_| DbError::Decode("coins < 0".into()))?,
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

    pub fn is_admin(&self) -> bool {
        matches!(self.role, AccountRole::Admin)
    }
}

pub struct UserRealmData {
    /// Account ID of the user
    pub account_id: AccountId,
    /// Realm ID of the user
    pub realm_id: RealmId,
    /// Current room ID of the user
    pub current_room_id: RoomId,
}

impl UserRealmData {
    pub fn try_from_row(row: &Row) -> DbResult<Self> {
        Ok(Self {
            account_id: row.try_get::<_, AccountId>("account_id")?,
            realm_id: row.try_get::<_, RealmId>("realm_id")?,
            current_room_id: row.try_get::<_, RoomId>("current_room_id")?,
        })
    }
}
