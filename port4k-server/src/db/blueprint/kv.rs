use super::super::Db;

impl Db {
    pub async fn bp_room_kv_get(
        &self,
        bp: &str,
        room: &str,
        key: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT value FROM blueprint_room_kv
                    WHERE bp_key=$1 AND room_key=$2 AND key=$3",
                &[&bp, &room, &key],
            )
            .await?;
        Ok(row.map(|r| r.get::<_, serde_json::Value>(0)))
    }

    pub async fn bp_room_kv_set(
        &self,
        bp: &str,
        room: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO blueprint_room_kv (bp_key, room_key, key, value)
                VALUES ($1,$2,$3,$4)
                ON CONFLICT (bp_key,room_key,key)
                DO UPDATE SET value=EXCLUDED.value",
            &[&bp, &room, &key, value],
        )
        .await?;
        Ok(())
    }

    pub async fn bp_player_kv_get(
        &self,
        bp: &str,
        account: &str,
        room: &str,
        key: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT value FROM blueprint_player_kv
                    WHERE bp_key=$1 AND account_name=$2 AND room_key=$3 AND key=$4",
                &[&bp, &account, &room, &key],
            )
            .await?;
        Ok(row.map(|r| r.get::<_, serde_json::Value>(0)))
    }

    pub async fn bp_player_kv_set(
        &self,
        bp: &str,
        account: &str,
        room: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO blueprint_player_kv (bp_key, account_name, room_key, key, value)
                VALUES ($1,$2,$3,$4,$5)
                ON CONFLICT (bp_key,account_name,room_key,key)
                DO UPDATE SET value=EXCLUDED.value",
            &[&bp, &account, &room, &key, value],
        )
        .await?;
        Ok(())
    }
}
