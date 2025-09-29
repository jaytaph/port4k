use super::Db;

impl Db {
    pub async fn room_view(&self, room_id: i64) -> anyhow::Result<String> {
        let client = self.pool.get().await?;
        let r = client
            .query_one("SELECT title, body FROM rooms WHERE id=$1", &[&room_id])
            .await?;
        let title: String = r.get(0);
        let body: String = r.get(1);

        let exits = client
            .query(
                "SELECT dir FROM exits WHERE from_room=$1 ORDER BY dir",
                &[&room_id],
            )
            .await?;
        let dirs: Vec<String> = exits.into_iter().map(|row| row.get::<_, String>(0)).collect();
        let exits_line = if dirs.is_empty() {
            "Exits: none".to_string()
        } else {
            format!("Exits: {}", dirs.join(", "))
        };
        Ok(format!("{title}\n{body}\n{exits_line}\n"))
    }

    pub async fn room_coin_total(&self, room_id: i64) -> anyhow::Result<i64> {
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
