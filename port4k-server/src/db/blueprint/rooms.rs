use super::super::Db;

impl Db {
    pub async fn bp_new(&self, bp_key: &str, title: &str, owner: &str) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "INSERT INTO blueprints (key, title, owner)
             VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
                &[&bp_key, &title, &owner],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_room_add(
        &self,
        bp_key: &str,
        room_key: &str,
        title: &str,
        body: &str,
    ) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "INSERT INTO blueprint_rooms (bp_key, key, title, body)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT DO NOTHING",
                &[&bp_key, &room_key, &title, &body],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_exit_add(
        &self,
        bp_key: &str,
        from_key: &str,
        dir: &str,
        to_key: &str,
    ) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "INSERT INTO blueprint_exits (bp_key, from_key, dir, to_key)
             VALUES ($1, $2, LOWER($3), $4)
             ON CONFLICT DO NOTHING",
                &[&bp_key, &from_key, &dir, &to_key],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_set_entry(&self, bp_key: &str, room_key: &str) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "UPDATE blueprints SET entry_room_key = $2 WHERE key = $1",
                &[&bp_key, &room_key],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_entry(&self, bp_key: &str) -> anyhow::Result<Option<String>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT entry_room_key FROM blueprints WHERE key=$1",
                &[&bp_key],
            )
            .await?;
        Ok(row.and_then(|r| r.get::<_, Option<String>>(0)))
    }

    pub async fn bp_room_view(
        &self,
        bp_key: &str,
        room_key: &str,
    ) -> anyhow::Result<Option<String>> {
        let c = self.pool.get().await?;
        let r = c
            .query_opt(
                "SELECT title, body FROM blueprint_rooms WHERE bp_key=$1 AND key=$2",
                &[&bp_key, &room_key],
            )
            .await?;
        let Some(r) = r else {
            return Ok(None);
        };
        let title: String = r.get(0);
        let body: String = r.get(1);

        let exits = c
            .query(
                "SELECT dir FROM blueprint_exits
                 WHERE bp_key=$1 AND from_key=$2
                 ORDER BY dir",
                &[&bp_key, &room_key],
            )
            .await?;
        let dirs: Vec<String> = exits
            .into_iter()
            .map(|row| row.get::<_, String>(0))
            .collect();
        let exits_line = if dirs.is_empty() {
            "Exits: none".to_string()
        } else {
            format!("Exits: {}", dirs.join(", "))
        };
        Ok(Some(format!("{title}\n{body}\n{exits_line}\n")))
    }

    pub async fn bp_move(
        &self,
        bp_key: &str,
        from_key: &str,
        dir: &str,
    ) -> anyhow::Result<Option<String>> {
        let c = self.pool.get().await?;
        let to = c
            .query_opt(
                "SELECT to_key FROM blueprint_exits
                 WHERE bp_key=$1 AND from_key=$2 AND dir=LOWER($3)",
                &[&bp_key, &from_key, &dir],
            )
            .await?;
        Ok(to.map(|r| r.get::<_, String>(0)))
    }
}
