#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(unused)]
pub struct AccountName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(unused)]
pub struct BlueprintKey(pub String);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct RoomId(pub i64);

impl From<i64> for RoomId {
    fn from(v: i64) -> Self {
        RoomId(v)
    }
}
impl From<RoomId> for i64 {
    fn from(v: RoomId) -> i64 {
        v.0
    }
}
