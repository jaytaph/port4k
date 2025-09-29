use super::Db;

impl Db {
    pub async fn start_room_id(&self) -> anyhow::Result<i64> {
        let client = self.pool.get().await?;
        let row = client
            .query_one(
                "SELECT r.id
                 FROM rooms r
                 JOIN zones z ON z.id = r.zone_id
                 WHERE z.key = 'start' AND r.key = 'entry'",
                &[],
            )
            .await?;
        Ok(row.get::<_, i64>(0))
    }

    pub async fn get_or_create_character(&self, account: &str) -> anyhow::Result<(i64, i64)> {
        let client = self.pool.get().await?;

        if let Some(row) = client
            .query_opt(
                "SELECT id, location_id
                 FROM characters
                 WHERE account_name = $1
                 ORDER BY id
                 LIMIT 1",
                &[&account],
            )
            .await?
        {
            let id: i64 = row.get(0);
            let loc: Option<i64> = row.get(1);
            let loc = if let Some(l) = loc {
                l
            } else {
                let s = self.start_room_id().await?;
                client
                    .execute(
                        "UPDATE characters SET location_id=$1 WHERE id=$2",
                        &[&s, &id],
                    )
                    .await?;
                s
            };
            return Ok((id, loc));
        }

        let loc = self.start_room_id().await?;
        let name = account;
        let row = client
            .query_one(
                "INSERT INTO characters (account_name, name, location_id)
                 VALUES ($1, $2, $3)
                 RETURNING id, location_id",
                &[&account, &name, &loc],
            )
            .await?;
        Ok((row.get(0), row.get(1)))
    }

    pub async fn move_character(&self, account: &str, dir: &str) -> anyhow::Result<Option<i64>> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT c.id, c.location_id
                 FROM characters c
                 WHERE c.account_name=$1
                 ORDER BY c.id
                 LIMIT 1",
                &[&account],
            )
            .await?;
        let Some(row) = row else { return Ok(None); };
        let cid: i64 = row.get(0);
        let cur: i64 = row.get(1);

        let to = client
            .query_opt(
                "SELECT to_room
                 FROM exits
                 WHERE from_room=$1 AND dir=LOWER($2)",
                &[&cur, &dir],
            )
            .await?;
        let Some(to_row) = to else { return Ok(None); };
        let new_room: i64 = to_row.get(0);

        client
            .execute(
                "UPDATE characters SET location_id=$1 WHERE id=$2",
                &[&new_room, &cid],
            )
            .await?;
        Ok(Some(new_room))
    }
}
