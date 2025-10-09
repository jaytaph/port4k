use crate::db::DbResult;
use crate::models::types::{AccountId, RoomId};
use super::super::Db;

impl Db {
    pub async fn bp_script_put_draft(
        &self,
        room_id: RoomId,
        author_id: AccountId,
        event: &str,
        source: &str,
    ) -> DbResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO bp_scripts_draft (room_id, event, source, author)
             VALUES ($1,$2,$3,$4)
             ON CONFLICT (room_id,event)
             DO UPDATE
             SET source=EXCLUDED.source, author=EXCLUDED.author, updated_at=now()",
            &[&room_id, &event, &source, &author_id],
        )
        .await?;
        Ok(())
    }

    pub async fn bp_script_publish(&self, room_id: RoomId, event: &str) -> DbResult<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "INSERT INTO bp_scripts_live (room_id, event, source, author, updated_at)
             SELECT room_id, event, source, author, now()
             FROM bp_scripts_draft
             WHERE room_id=$1 AND event=$2
             ON CONFLICT (room_id,event)
             DO UPDATE
             SET source=EXCLUDED.source, author=EXCLUDED.author, updated_at=now()",
                &[&room_id, &event],
            )
            .await?;
        Ok(n > 0)
    }

    pub async fn bp_script_get_live(
        &self,
        room_id: RoomId,
        event: &str,
    ) -> DbResult<Option<String>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT source FROM bp_scripts_live WHERE room_id=$1 AND event=$2",
                &[&room_id, &event],
            )
            .await?;
        Ok(row.map(|r| r.get::<_, String>(0)))
    }
}
