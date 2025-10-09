use crate::models::types::RoomId;
use super::{Db, DbResult};

impl Db {
    pub async fn room_view(&self, room_id: RoomId) -> DbResult<String> {
        let client = self.pool.get().await?;

        // bp_rooms has (id, title, body, ...)
        let r = client
            .query_one(
                "SELECT title, body FROM bp_rooms WHERE id = $1",
                &[&room_id],
            )
            .await?;
        let title: String = r.get(0);
        let body: String = r.get(1);

        // bp_exits uses from_room_id; hide exits that are locked AND not visible_when_locked
        let rows = client
            .query(
                "SELECT dir
             FROM bp_exits
             WHERE from_room_id = $1
               AND (visible_when_locked OR NOT locked)
             ORDER BY dir",
                &[&room_id],
            )
            .await?;

        let dirs: Vec<String> = rows.into_iter().map(|row| row.get(0)).collect();
        let exits_line = if dirs.is_empty() {
            "Exits: none".to_string()
        } else {
            format!("Exits: {}", dirs.join(", "))
        };

        Ok(format!("{title}\n{body}\n{exits_line}\n"))
    }

    #[allow(unused)]
    pub async fn room_coin_total(&self, room_id: i64) -> DbResult<i64> {
        let client = self.pool.get().await?;
        let row = client
            .query_one(
                "SELECT COALESCE(SUM(qty), 0)
                 FROM room_loot
                 WHERE room_id = $1 AND item = 'coin' AND picked_by IS NULL",
                &[&room_id],
            )
            .await?;
        Ok(row.get(0))
    }
}
