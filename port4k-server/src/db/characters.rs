use super::{Db, DbResult};
use crate::models::types::{AccountId, CharacterId, RoomId};

impl Db {
    /// Returns the room ID of the starting room ("start" zone, "entry" room).
    pub async fn start_room_id(&self) -> DbResult<RoomId> {
        let client = self.pool.get().await?;
        let row = client
            .query_one(
                r#"
            SELECT r.id
            FROM bp_rooms r
            JOIN blueprints b ON b.id = r.bp_id
            JOIN zones z ON z.key = b.key
            WHERE z.key = 'hub' AND r.key = 'entry'
            "#,
                &[],
            )
            .await?;
        Ok(row.get(0))
    }

    pub async fn get_or_create_character(
        &self,
        account_id: AccountId,
        username: &str,
    ) -> DbResult<(CharacterId, RoomId)> {
        let client = self.pool.get().await?;

        if let Some(row) = client
            .query_opt(
                "SELECT id, room_id
                 FROM characters
                 WHERE account_id = $1
                 LIMIT 1",
                &[&account_id],
            )
            .await?
        {
            let character_id: CharacterId = row.get(0);
            let loc: Option<RoomId> = row.get(1);
            let loc = if let Some(l) = loc {
                l
            } else {
                let s = self.start_room_id().await?;
                client
                    .execute("UPDATE characters SET room_id=$1 WHERE id=$2", &[&s, &character_id])
                    .await?;
                s
            };

            return Ok((character_id, loc));
        }

        let loc = self.start_room_id().await?;
        let row = client
            .query_one(
                "INSERT INTO characters (account_id, name, room_id)
                 VALUES ($1, $2, $3)
                 RETURNING id, room_id",
                &[&account_id, &username, &loc],
            )
            .await?;

        Ok((row.get(0), row.get(1)))
    }

    pub async fn move_character(&self, account_id: AccountId, dir: &str) -> DbResult<Option<RoomId>> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT c.id, c.room_id
                 FROM characters c
                 WHERE c.account_id=$1
                 ORDER BY c.id
                 LIMIT 1",
                &[&account_id],
            )
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let character_id: i64 = row.get(0);
        let cur_room: i64 = row.get(1);

        let to = client
            .query_opt(
                "SELECT to_room
                 FROM exits
                 WHERE from_room=$1 AND dir=LOWER($2)",
                &[&cur_room, &dir],
            )
            .await?;
        let Some(to_row) = to else {
            return Ok(None);
        };
        let new_room: RoomId = to_row.get(0);

        client
            .execute(
                "UPDATE characters SET room_id=$1 WHERE id=$2",
                &[&new_room, &character_id],
            )
            .await?;
        Ok(Some(new_room))
    }
}
