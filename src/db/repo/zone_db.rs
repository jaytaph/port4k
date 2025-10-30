use crate::db::error::DbError;
use crate::db::repo::zone::ZoneRepo;
use crate::db::{Db, DbResult};
use crate::models::room::Kv;
use crate::models::types::{ExitId, ObjectId, RoomId, ZoneId};
use crate::models::zone::Zone;
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::Value;

pub struct ZoneRepository {
    db: Arc<Db>,
}

impl ZoneRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db: db.clone() }
    }
}

#[async_trait::async_trait]
impl ZoneRepo for ZoneRepository {
    async fn get_by_key(&self, zone_key: &str) -> DbResult<Option<Zone>> {
        let client = self.db.get_client().await?;

        let stmt = client
            .prepare_cached(
                r#"
            SELECT id, key, title, kind, created_at
            FROM zones
            WHERE key = $1
        "#,
            )
            .await?;

        let row_opt = client.query_opt(&stmt, &[&zone_key]).await?;
        row_opt.as_ref().map(Zone::try_from_row).transpose()
    }

    async fn room_kv(&self, zone_id: ZoneId, room_id: RoomId) -> DbResult<Kv> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
                SELECT key, value FROM zone_room_kv
                WHERE zone_id = $1 AND room_id = $2
                "#,
                &[&zone_id, &room_id],
            )
            .await?;

        Ok(Kv::try_from_rows(&rows).map_err(|_| DbError::Decode("Cannot decode row to kv".into()))?)
    }

    async fn obj_kv(&self, zone_id: ZoneId, room_id: RoomId) -> DbResult<HashMap<String, Kv>> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
                SELECT o.name AS object_key, key, value FROM zone_object_kv
                JOIN bp_objects o ON o.id = zone_object_kv.object_id
                WHERE zone_id = $1 AND room_id = $2
                "#,
                &[&zone_id, &room_id],
            )
            .await?;

        let mut map: HashMap<String, Kv> = HashMap::new();

        for row in rows {
            let object_key: String = row.get("object_key");
            let kv_key: String = row.get("kv_key");
            let value: serde_json::Value = row.get("value");

            map.entry(object_key).or_default().insert(kv_key, value);
        }

        Ok(map)
    }

    async fn set_object_kv(&self, zone_id: ZoneId, object_id: ObjectId, key: &str, value: &Value) -> DbResult<()> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
                INSERT INTO zone_object_kv (zone_id, object_id, key, value)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (zone_id, account_id, object_id, key)
                DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&zone_id, &object_id, &key, &value],
            )
            .await?;

        Ok(())
    }

    async fn set_exit_locked(&self, zone_id: ZoneId, room_id: RoomId, exit_id: ExitId, locked: bool) -> DbResult<()> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
                INSERT INTO zone_exits (zone_id, room_id, exit_id, locked)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (zone_id, room_id, exit_id)
                DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&zone_id, &room_id, &exit_id, &locked],
            )
            .await?;

        Ok(())
    }
}
