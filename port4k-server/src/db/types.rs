use postgres_types::{FromSql, ToSql};

// #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ToSql, FromSql)]
// #[repr(transparent)]
// #[postgres(transparent)]
// pub struct RoomId(pub uuid::Uuid);
//
// #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ToSql, FromSql)]
// #[repr(transparent)]
// #[postgres(transparent)]
// pub struct AccountId(pub uuid::Uuid);
//
// #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ToSql, FromSql)]
// #[repr(transparent)]
// #[postgres(transparent)]
// pub struct CharacterId(pub uuid::Uuid);
//
// #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ToSql, FromSql)]
// #[repr(transparent)]
// #[postgres(transparent)]
// pub struct BluePrintId(pub uuid::Uuid);
//
// #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ToSql, FromSql)]
// #[repr(transparent)]
// #[postgres(transparent)]
// pub struct ZoneId(pub uuid::Uuid);

#[macro_export]
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ToSql, FromSql)]
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
define_id!(ZoneId);
define_id!(BlueprintId);
define_id!(RoomId);
define_id!(ObjectId);
define_id!(CharacterId);
define_id!(LootId);