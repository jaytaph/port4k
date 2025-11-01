use crate::db::error::DbError;
use crate::db::repo::realm::RealmRepo;
use crate::db::{Db, DbResult, map_row, map_row_opt};
use crate::models::realm::Realm;
use crate::models::room::Kv;
use crate::models::types::{AccountId, ExitId, ObjectId, RealmId, RoomId};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub struct RealmRepository {
    db: Arc<Db>,
}

impl RealmRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db: db.clone() }
    }
}

#[async_trait::async_trait]
impl RealmRepo for RealmRepository {
    async fn get(&self, realm_id: RealmId) -> DbResult<Option<Realm>> {
        let client = self.db.get_client().await?;
        let stmt = client
            .prepare_cached(
                r#"
            SELECT id, bp_id, title, kind, created_at
            FROM realms
            WHERE id = $1
        "#,
            )
            .await?;
        let row_opt = client.query_opt(&stmt, &[&realm_id]).await?;
        row_opt.as_ref().map(Realm::try_from_row).transpose()
    }

    async fn get_by_key(&self, key: &str) -> DbResult<Option<Realm>> {
        let client = self.db.get_client().await?;

        let rows = client
            .query_opt(
                r#"
                    SELECT id, bp_id, key, title, kind, created_at, owner_id
                    FROM realms
                    WHERE key = $1
                "#,
                &[&key],
            )
            .await?;

        map_row_opt(rows, Realm::try_from_row, &format!("RealmRepo::get_by_key key={}", key))
    }

    async fn create(&self, realm: Realm) -> DbResult<Realm> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
            INSERT INTO realms (id, bp_id, title, kind, created_at)
            VALUES ($1, $2, $3, $4, $5)
        "#,
                &[
                    &realm.id,
                    &realm.bp_id,
                    &realm.title,
                    &realm.kind.to_string(),
                    &realm.created_at,
                ],
            )
            .await?;

        Ok(realm)
    }

    async fn find_by_owner(&self, owner_id: AccountId) -> DbResult<Vec<Realm>> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
            SELECT id, bp_id, title, kind, created_at
            FROM realms
            WHERE kind->>'owner' = $1
        "#,
                &[&owner_id],
            )
            .await?;

        let realms: DbResult<Vec<Realm>> = rows
            .into_iter()
            .map(|row| {
                map_row(
                    &row,
                    Realm::try_from_row,
                    &format!("RealmRepo::find_by_owner owner_id={}", owner_id),
                )
            })
            .collect();

        realms
    }

    async fn room_kv(&self, realm_id: RealmId, room_id: RoomId) -> DbResult<Kv> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
                SELECT key, value FROM realm_room_kv
                WHERE realm_id = $1 AND room_id = $2
                "#,
                &[&realm_id, &room_id],
            )
            .await?;

        Ok(Kv::try_from_rows(&rows).map_err(|_| DbError::Decode("Cannot decode row to kv".into()))?)
    }

    async fn obj_kv(&self, realm_id: RealmId, room_id: RoomId) -> DbResult<HashMap<String, Kv>> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
                SELECT o.name AS object_key, key, value FROM realm_object_kv
                JOIN bp_objects o ON o.id = realm_object_kv.object_id
                WHERE realm_id = $1 AND room_id = $2
                "#,
                &[&realm_id, &room_id],
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

    async fn set_room_kv(&self, realm_id: RealmId, room_id: RoomId, key: &str, value: &Value) -> DbResult<()> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
                INSERT INTO realm_room_kv (realm_id, room_id, key, value)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (realm_id, room_id, key)
                DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&realm_id, &room_id, &key, &value],
            )
            .await?;

        Ok(())
    }

    async fn set_object_kv(&self, realm_id: RealmId, object_id: ObjectId, key: &str, value: &Value) -> DbResult<()> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
                INSERT INTO realm_object_kv (realm_id, object_id, key, value)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (realm_id, account_id, object_id, key)
                DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&realm_id, &object_id, &key, &value],
            )
            .await?;

        Ok(())
    }

    async fn set_exit_locked(&self, realm_id: RealmId, room_id: RoomId, exit_id: ExitId, locked: bool) -> DbResult<()> {
        let client = self.db.get_client().await?;

        client
            .execute(
                r#"
                INSERT INTO realm_exits (realm_id, room_id, exit_id, locked)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (realm_id, room_id, exit_id)
                DO UPDATE SET value = EXCLUDED.value
                "#,
                &[&realm_id, &room_id, &exit_id, &locked],
            )
            .await?;

        Ok(())
    }
}
