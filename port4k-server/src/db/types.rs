use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};

#[macro_export]
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ToSql, FromSql, Serialize, Deserialize)]
        #[repr(transparent)]
        #[postgres(transparent)]
        pub struct $name(pub uuid::Uuid);

        impl $name {
            #[inline] pub fn new() -> Self { Self(uuid::Uuid::new_v4()) }
            #[inline] pub fn from_uuid(u: uuid::Uuid) -> Self { Self(u) }
            #[inline] pub fn as_uuid(&self) -> &uuid::Uuid { &self.0 }
        }

        impl From<uuid::Uuid> for $name {
            #[inline] fn from(value: uuid::Uuid) -> Self { Self(value) }
        }
        impl From<$name> for uuid::Uuid {
            #[inline] fn from(value: $name) -> uuid::Uuid { value.0 }
        }
        impl AsRef<uuid::Uuid> for $name {
            #[inline] fn as_ref(&self) -> &uuid::Uuid { &self.0 }
        }

        impl std::fmt::Display for $name {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            #[inline]
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                uuid::Uuid::parse_str(s).map(Self)
            }
        }
    };
}

define_id!(AccountId);
define_id!(CharacterId);
define_id!(ZoneId);
define_id!(BlueprintId);
define_id!(RoomId);
define_id!(ObjectId);
define_id!(LootId);


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScriptSource {
    Live,
    Draft,
}

/// Directions as used in `bp_exits.dir`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    North, South, East, West, Up, Down,
    Northeast, Northwest, Southeast, Southwest,
    Custom(String), // fallback for user-defined directions
}

impl From<String> for Direction {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "n" | "north" => Direction::North,
            "s" | "south" => Direction::South,
            "e" | "east"  => Direction::East,
            "w" | "west"  => Direction::West,
            "u" | "up"    => Direction::Up,
            "d" | "down"  => Direction::Down,
            "ne" | "northeast" => Direction::Northeast,
            "nw" | "northwest" => Direction::Northwest,
            "se" | "southeast" => Direction::Southeast,
            "sw" | "southwest" => Direction::Southwest,
            other => Direction::Custom(other.to_string()),
        }
    }
}