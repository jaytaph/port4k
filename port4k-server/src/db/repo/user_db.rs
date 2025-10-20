use std::collections::HashMap;
use crate::db::{Db, DbResult};
use std::sync::Arc;
use crate::db::error::DbError;
use crate::db::repo::UserRepo;
use crate::models::room::Kv;
use crate::models::types::{AccountId, RoomId, ZoneId};
use crate::util::serde::serde_to_str;

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
                SELECT object_key, key, value FROM user_object_kv
                WHERE zone_id = $1 AND room_id = $2 AND account_id = $3
                "#,
                &[&zone_id, &room_id, &account_id],
            )
            .await?;


        let mut map: HashMap<String, Kv> = HashMap::new();

        for row in rows {
            let object_key: String = row.get("object_key");
            let kv_key: String = row.get("kv_key");
            let value: serde_json::Value = row.get("value");

            map.entry(object_key)
                .or_default()
                .insert(kv_key, serde_to_str(value));
        }

        Ok(map)
    }
}

