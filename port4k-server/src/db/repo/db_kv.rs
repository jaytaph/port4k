use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{Map, Value};
use crate::db::{Db, DbResult};
use crate::db::repo::kv::KvRepo;
use crate::models::types::{AccountId, RoomId};

pub struct KvRepository {
    pub db: Arc<Db>,
}

impl KvRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db: db.clone() }
    }

}

#[async_trait]
impl KvRepo for KvRepository {
    async fn room_kv_get_all(&self, room_id: RoomId) -> DbResult<Map<String, Value>> {
        let client = self.db.get_client().await?;
        let rows = client
            .query(
                "SELECT key, value
                 FROM bp_room_kv
                 WHERE room_id = $1",
                &[&room_id.0],
            )
            .await?;

        let mut map = Map::new();
        for row in rows {
            let key: String = row.get(0);
            let value: Value = row.get(1);
            map.insert(key, value);
        }
        Ok(map)
    }

    // Returns Value::Null if key isn't present (signature isn't Option).
    async fn room_kv_get(&self, room_id: RoomId, obj_key: &str) -> DbResult<Value> {
        let client = self.db.get_client().await?;
        let row_opt = client
            .query_opt(
                "SELECT value
                FROM bp_room_kv
                WHERE room_id = $1 AND key = $2",
                &[&room_id.0, &obj_key],
            )
            .await?;

        Ok(row_opt.map(|r| r.get::<_, Value>(0)).unwrap_or(Value::Null))
    }

    // Upsert; returns true if inserted, false if updated.
    async fn room_kv_set(&self, room_id: RoomId, obj_key: &str, value: Value) -> DbResult<bool> {
        let client = self.db.get_client().await?;
        let row = client
            .query_one(
                "INSERT INTO bp_room_kv (room_id, key, value)
                VALUES ($1, $2, $3)
                ON CONFLICT (room_id, key) DO UPDATE SET value = EXCLUDED.value
                RETURNING (xmax = 0) AS inserted",
                &[&room_id.0, &obj_key, &value],
            )
            .await?;
        let inserted: bool = row.get("inserted");
        Ok(inserted)
    }

    // Returns None if no row exists.
    async fn player_kv_get(&self, room_id: RoomId, account_id: AccountId, obj_key: &str) -> DbResult<Option<Value>> {
        let client = self.db.get_client().await?;
        let row_opt = client
            .query_opt(
                "SELECT value
                FROM bp_player_kv
                WHERE room_id = $1 AND account_id = $2 AND key = $3",
                &[&room_id.0, &account_id.0, &obj_key],
            )
            .await?;
        Ok(row_opt.map(|r| r.get::<_, Value>(0)))
    }

    // Upsert; returns true if inserted, false if updated.
    async fn player_kv_set(&self, room_id: RoomId, account_id: AccountId, obj_key: &str, value: Value) -> DbResult<bool> {
        let client = self.db.get_client().await?;
        let row = client
            .query_one(
                "INSERT INTO bp_player_kv (room_id, account_id, key, value)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (room_id, account_id, key) DO UPDATE SET value = EXCLUDED.value
                RETURNING (xmax = 0) AS inserted",
                &[&room_id.0, &account_id.0, &obj_key, &value],
            )
            .await?;
        let inserted: bool = row.get("inserted");
        Ok(inserted)
    }
}
