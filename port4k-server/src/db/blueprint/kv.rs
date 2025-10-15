use super::super::Db;
use crate::db::DbResult;
use crate::models::types::{AccountId, RoomId};

impl Db {
    pub async fn bp_room_kv_get(&self, room_id: RoomId, key: &str) -> DbResult<Option<serde_json::Value>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT value FROM bp_room_kv
                    WHERE room_id=$1 AND key=$2",
                &[&room_id, &key],
            )
            .await?;
        Ok(row.map(|r| r.get::<_, serde_json::Value>(0)))
    }

    pub async fn bp_room_kv_set(&self, room_id: RoomId, key: &str, value: &serde_json::Value) -> DbResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO bp_room_kv (room_id, key, value)
                VALUES ($1,$2,$3)
                ON CONFLICT (room_id, key)
                DO UPDATE SET value=EXCLUDED.value",
            &[&room_id, &key, value],
        )
        .await?;
        Ok(())
    }

    pub async fn bp_player_kv_get(
        &self,
        account_id: AccountId,
        room_id: RoomId,
        key: &str,
    ) -> DbResult<Option<serde_json::Value>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT value FROM bp_player_kv
                    WHERE room_id=$1 AND account_id=$2 AND key=$4",
                &[&room_id, &account_id, &key],
            )
            .await?;
        Ok(row.map(|r| r.get::<_, serde_json::Value>(0)))
    }

    pub async fn bp_player_kv_set(
        &self,
        account_id: AccountId,
        room_id: RoomId,
        key: &str,
        value: &serde_json::Value,
    ) -> DbResult<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO bp_player_kv (room_id, account_id, key, value)
                VALUES ($1,$2,$3,$4)
                ON CONFLICT (room_id,account_id,key)
                DO UPDATE SET value=EXCLUDED.value",
            &[&room_id, &account_id, &key, value],
        )
        .await?;
        Ok(())
    }
}
