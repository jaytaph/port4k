use std::fmt::Display;
use postgres_types::{FromSql, ToSql};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(unused)]
pub struct AccountName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(unused)]
pub struct BlueprintKey(pub String);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RoomId(pub i64);

impl Display for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromSql<'_> for RoomId {
    fn from_sql(
        ty: &postgres_types::Type,
        raw: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let v = i64::from_sql(ty, raw)?;
        Ok(RoomId(v))
    }

    fn accepts(ty: &postgres_types::Type) -> bool {
        <i64 as FromSql>::accepts(ty)
    }
}

impl ToSql for RoomId {
    fn to_sql(
        &self,
        ty: &postgres_types::Type,
        out: &mut bytes::BytesMut,
    ) -> Result<postgres_types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        self.0.to_sql(ty, out)
    }

    fn accepts(ty: &postgres_types::Type) -> bool {
        <i64 as ToSql>::accepts(ty)
    }

    postgres_types::to_sql_checked!();
}

impl Into<RoomId> for i64 { fn into(self) -> RoomId { RoomId(self) } }
impl From<RoomId> for i64 { fn from(v: RoomId) -> i64 { v.0 } }
impl From<&RoomId> for i64 { fn from(v: &RoomId) -> i64 { v.0 } }
