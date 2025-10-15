// use tokio_postgres::Row;
// use crate::db::DbResult;
// use crate::models::{json_string_vec_opt};
// use crate::models::types::{AccountId, CharacterId, RoomId, ZoneId};
//
// #[derive(Debug, Clone)]
// pub struct Character {
//     pub id: CharacterId,
//     pub account_id: AccountId,
//     pub name: String,
//     pub zone_id: ZoneId,
//     pub room_id: RoomId,
//     pub stats: Vec<String>,
//     pub created_at: chrono::DateTime<chrono::Utc>,
// }
//
// impl Character {
//     pub fn try_from_row(row: &Row) -> DbResult<Self> {
//         let stats = json_string_vec_opt(
//             row.try_get::<_, Option<serde_json::Value>>("stats")?,
//             "stats"
//         )?;
//
//         Ok(Self {
//             id: row.try_get::<_, CharacterId>("id")?,
//             account_id: row.try_get::<_, AccountId>("account_id")?,
//             name: row.try_get("name")?,
//             zone_id: row.try_get::<_, ZoneId>("zone_id")?,
//             room_id: row.try_get::<_, RoomId>("room_id")?,
//             stats,
//             created_at: row.try_get("created_at")?,
//         })
//     }
// }
