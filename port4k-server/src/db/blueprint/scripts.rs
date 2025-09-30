use super::super::Db;

impl Db {
    pub async fn bp_script_put_draft(
        &self,
        bp_key: &str,
        room_key: &str,
        event: &str,
        source: &str,
        author: &str,
    ) -> anyhow::Result<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO bp_scripts_draft (bp_key, room_key, event, source, author)
             VALUES ($1,$2,$3,$4,$5)
             ON CONFLICT (bp_key,room_key,event)
             DO UPDATE
             SET source=EXCLUDED.source, author=EXCLUDED.author, updated_at=now()",
            &[&bp_key, &room_key, &event, &source, &author],
        )
        .await?;
        Ok(())
    }

    pub async fn bp_script_publish(
        &self,
        bp_key: &str,
        room_key: &str,
        event: &str,
    ) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c.execute(
            "INSERT INTO bp_scripts_live (bp_key, room_key, event, source, author, updated_at)
             SELECT bp_key, room_key, event, source, author, now()
             FROM bp_scripts_draft
             WHERE bp_key=$1 AND room_key=$2 AND event=$3
             ON CONFLICT (bp_key,room_key,event)
             DO UPDATE
             SET source=EXCLUDED.source, author=EXCLUDED.author, updated_at=now()",
            &[&bp_key, &room_key, &event],
        )
            .await?;
        Ok(n > 0)
    }

    pub async fn bp_script_get_live(
        &self,
        bp_key: &str,
        room_key: &str,
        event: &str,
    ) -> anyhow::Result<Option<String>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT source FROM bp_scripts_live WHERE bp_key=$1 AND room_key=$2 AND event=$3",
                &[&bp_key, &room_key, &event],
            )
            .await?;
        Ok(row.map(|r| r.get::<_, String>(0)))
    }
}
