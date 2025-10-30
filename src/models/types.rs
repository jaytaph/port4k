use crate::error::DomainError;
use serde::{Deserialize, Serialize};

#[macro_export]
macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Copy,
            Clone,
            Debug,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            postgres_types::ToSql,
            postgres_types::FromSql,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[repr(transparent)]
        #[postgres(transparent)]
        #[serde(transparent)] // JSON = plain UUID string
        pub struct $name(pub uuid::Uuid);

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            #[inline]
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }
            #[inline]
            pub fn from_uuid(u: uuid::Uuid) -> Self {
                Self(u)
            }
            #[inline]
            pub fn as_uuid(&self) -> &uuid::Uuid {
                &self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl core::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                uuid::Uuid::parse_str(s).map(Self)
            }
        }

        impl core::convert::TryFrom<&str> for $name {
            type Error = uuid::Error;
            fn try_from(s: &str) -> Result<Self, Self::Error> {
                s.parse()
            }
        }

        impl From<uuid::Uuid> for $name {
            fn from(v: uuid::Uuid) -> Self {
                Self(v)
            }
        }
        impl From<$name> for uuid::Uuid {
            fn from(v: $name) -> uuid::Uuid {
                v.0
            }
        }
        impl AsRef<uuid::Uuid> for $name {
            fn as_ref(&self) -> &uuid::Uuid {
                &self.0
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
define_id!(ExitId);
define_id!(LootId);
define_id!(HintId);
define_id!(ItemId);

/// Directions as used in `bp_exits.dir`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    North,
    South,
    East,
    West,
    Up,
    Down,
    In,
    Out,
    Northeast,
    Northwest,
    Southeast,
    Southwest,
    Custom(String), // for user-defined directions
}

impl Direction {
    pub fn as_str(&self) -> &str {
        match self {
            Direction::Custom(s) => s.as_str(),
            _ => self.canonical(),
        }
    }

    #[inline]
    pub fn canonical(&self) -> &'static str {
        match self {
            Direction::North => "north",
            Direction::South => "south",
            Direction::East => "east",
            Direction::West => "west",
            Direction::Up => "up",
            Direction::Down => "down",
            Direction::In => "in",
            Direction::Out => "out",
            Direction::Northeast => "northeast",
            Direction::Northwest => "northwest",
            Direction::Southeast => "southeast",
            Direction::Southwest => "southwest",
            Direction::Custom(_) => "custom",
        }
    }

    pub fn to_short(&self) -> &'static str {
        match self {
            Direction::North => "n",
            Direction::South => "s",
            Direction::East => "e",
            Direction::West => "w",
            Direction::Up => "u",
            Direction::Down => "d",
            Direction::In => "i",
            Direction::Out => "o",
            Direction::Northeast => "ne",
            Direction::Northwest => "nw",
            Direction::Southeast => "se",
            Direction::Southwest => "sw",
            Direction::Custom(_) => "c",
        }
    }
}

impl core::str::FromStr for Direction {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "north" => Ok(Direction::North),
            "south" => Ok(Direction::South),
            "east" => Ok(Direction::East),
            "west" => Ok(Direction::West),
            "up" => Ok(Direction::Up),
            "down" => Ok(Direction::Down),
            "in" => Ok(Direction::In),
            "out" => Ok(Direction::Out),
            "northeast" => Ok(Direction::Northeast),
            "northwest" => Ok(Direction::Northwest),
            "southeast" => Ok(Direction::Southeast),
            "southwest" => Ok(Direction::Southwest),
            _ => Err(DomainError::InvalidDirection(s.to_string())),
        }
    }
}

impl core::fmt::Display for Direction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Direction::Custom(s) => f.write_str(s),
            _ => f.write_str(self.canonical()),
        }
    }
}

impl From<Direction> for String {
    fn from(d: Direction) -> Self {
        d.to_string()
    }
}

impl Direction {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            // cardinal + aliases
            "n" | "north" => Some(Direction::North),
            "e" | "east" => Some(Direction::East),
            "s" | "south" => Some(Direction::South),
            "w" | "west" => Some(Direction::West),

            "ne" | "northeast" => Some(Direction::Northeast),
            "nw" | "northwest" => Some(Direction::Northwest),
            "se" | "southeast" => Some(Direction::Southeast),
            "sw" | "southwest" => Some(Direction::Southwest),

            "u" | "up" => Some(Direction::Up),
            "d" | "down" => Some(Direction::Down),
            "in" => Some(Direction::In),
            "out" => Some(Direction::Out),
            _ => None,
        }
    }
}
