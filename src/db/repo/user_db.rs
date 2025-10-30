use crate::db::error::DbError;
use crate::db::repo::UserRepo;
use crate::db::{Db, DbResult};
use crate::models::room::Kv;
use crate::models::types::{AccountId, ExitId, ObjectId, RoomId, ZoneId};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub struct UserRepository {
    db: Arc<Db>,
}

impl UserRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db: db.clone() }
    }
}

#[async_trait::async_trait]
impl UserRepo for UserRepository {
    async fn room_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId) -> DbResult<Kv> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
                SELECT key, value FROM user_room_kv
                WHERE zone_id = $1 AND room_id = $2 AND account_id = $3
                "#,
                &[&zone_id, &room_id, &account_id],
            )
            .await?;

        Ok(Kv::try_from_rows(&rows).map_err(|_| DbError::Decode("Cannot decode row to kv".into()))?)
    }

    async fn obj_kv(&self, zone_id: ZoneId, room_id: RoomId, account_id: AccountId) -> DbResult<HashMap<String, Kv>> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
                SELECT o.name AS object_key, u.key, u.value FROM user_object_kv AS u
                JOIN bp_objects o ON o.id = u.object_id
                WHERE o.room_id = $2 AND u.zone_id = $1 AND u.account_id = $3
                "#,
                &[&zone_id, &room_id, &account_id],
            )
            .await?;

        let mut map: HashMap<String, Kv> = HashMap::new();

        for row in rows {
            let object_key: String = row.get("object_key");
            let kv_key: String = row.get("kv_key");
            let value: Value = row.get("value");

            map.entry(object_key).or_default().insert(kv_key, value);
        }

        Ok(map)
    }

    async fn inc_room_kv(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        key: &str,
        inc_by: i64,
    ) -> DbResult<i64> {
        let client = self.db.get_client().await?;

        let row = client
            .query_one(
                r#"
            INSERT INTO user_room_kv (zone_id, room_id, account_id, key, value)
            VALUES ($1, $2, $3, $4, to_jsonb($5::bigint))
            ON CONFLICT (zone_id, room_id, account_id, key)
            DO UPDATE SET value = to_jsonb((COALESCE((user_room_kv.value->>0)::bigint, 0) + $5)::bigint)
            RETURNING value
            "#,
                &[&zone_id, &room_id, &account_id, &key, &inc_by],
            )
            .await?;

        let value: Value = row.get("value");
        let new_value = value
            .as_i64()
            .ok_or_else(|| DbError::Decode("Cannot decode incremented value".into()))?;

        Ok(new_value)
    }

    async fn set_room_kv(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        key: &str,
        value: &Value,
    ) -> DbResult<()> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
                INSERT INTO user_room_kv (zone_id, room_id, account_id, key, value)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (zone_id, room_id, account_id, key)
                DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&zone_id, &room_id, &account_id, &key, &value],
            )
            .await?;

        Ok(())
    }

    async fn set_object_kv(
        &self,
        zone_id: ZoneId,
        account_id: AccountId,
        object_id: ObjectId,
        key: &str,
        value: &Value,
    ) -> DbResult<()> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
                INSERT INTO user_object_kv (zone_id, account_id, object_id, key, value)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (zone_id, account_id, object_id, key)
                DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&zone_id, &account_id, &object_id, &key, &value],
            )
            .await?;

        Ok(())
    }

    async fn set_exit_locked(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        exit_id: ExitId,
        locked: bool,
    ) -> DbResult<()> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
                INSERT INTO user_exits (zone_id, room_id, account_id, exit_id, locked)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (zone_id, room_id, account_id, exit_id)
                DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&zone_id, &room_id, &account_id, &exit_id, &locked],
            )
            .await?;

        Ok(())
    }

    async fn is_exit_locked(
        &self,
        zone_id: ZoneId,
        room_id: RoomId,
        account_id: AccountId,
        exit_id: ExitId,
    ) -> DbResult<bool> {
        let client = self.db.get_client().await?;

        let row = client
            .query_one(
                r#"
                SELECT locked FROM user_exits
                WHERE zone_id = $1 AND room_id = $2 AND account_id = $3 AND exit_id = $4
                "#,
                &[&zone_id, &room_id, &account_id, &exit_id],
            )
            .await?;

        let locked: bool = row.get("locked");
        Ok(locked)
    }
}
