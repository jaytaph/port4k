use crate::db::error::DbError;
use crate::db::repo::{BlueprintAndRoomKey, RoomRepo};
use crate::db::{Db, DbResult};
use crate::lua::ScriptHook;
use crate::models::blueprint::Blueprint;
use crate::models::room::{BlueprintExit, BlueprintObject, BlueprintRoom, Kv, RoomScripts};
use crate::models::types::{AccountId, BlueprintId, RoomId};
use std::sync::Arc;

pub struct RoomRepository {
    pub db: Arc<Db>,
}

impl RoomRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db: db.clone() }
    }
}

#[async_trait::async_trait]
impl RoomRepo for RoomRepository {
    async fn blueprint_by_key(&self, bp_key: &str) -> DbResult<Blueprint> {
        let client = self.db.get_client().await?;

        let row = client
            .query_one(
                r#"
            SELECT id, key, title, owner_id, entry_room_id, status, created_at
            FROM blueprints
            WHERE key = $1
            "#,
                &[&bp_key],
            )
            .await?;

        Blueprint::try_from_row(&row)
    }

    async fn room_by_id(&self, bp_id: BlueprintId, room_id: RoomId) -> DbResult<BlueprintRoom> {
        let client = self.db.get_client().await?;

        let row = client
            .query_one(
                r#"
            SELECT r.id, r.bp_id, r.key, r.title, r.body, r.lockdown, r.short, r.hints
            FROM bp_rooms r
            WHERE r.id = $1 AND r.bp_id = $2
            "#,
                &[&room_id, &bp_id],
            )
            .await?;

        BlueprintRoom::try_from_row(&row)
    }

    async fn room_by_key(&self, key: &BlueprintAndRoomKey) -> DbResult<BlueprintRoom> {
        let client = self.db.get_client().await?;
        let row = client
            .query_one(
                r#"
            SELECT r.id, r.bp_id, r.key, r.title, r.body, r.lockdown, r.short, r.hints
            FROM bp_rooms r
            JOIN blueprints bp ON bp.id = r.bp_id
            WHERE bp.key = $1 AND r.key = $2
            "#,
                &[&key.bp_key, &key.room_key],
            )
            .await?;

        BlueprintRoom::try_from_row(&row)
    }

    async fn room_exits(&self, room_id: RoomId) -> DbResult<Vec<BlueprintExit>> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
                SELECT
                    e.id,
                    e.from_room_id,
                    fr.key AS from_room_key,
                    e.dir,
                    e.to_room_id,
                    tr.key AS to_room_key,
                    e.locked,
                    e.description,
                    e.visible_when_locked
                FROM bp_exits e
                JOIN bp_rooms fr ON e.from_room_id = fr.id
                JOIN bp_rooms tr ON e.to_room_id = tr.id
                WHERE e.from_room_id = $1
                ORDER BY e.dir;
                "#,
                &[&room_id],
            )
            .await?;

        let exits = rows
            .iter()
            .map(BlueprintExit::try_from_row)
            .collect::<DbResult<Vec<_>>>()?;
        Ok(exits)
    }

    async fn room_objects(&self, room_id: RoomId) -> DbResult<Vec<BlueprintObject>> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
        SELECT o.id, o.room_id, o.name, o.short, o.description, o.examine, o.flags, o.state, o.use_lua, o.position, o.loot,
            COALESCE(n.nouns, ARRAY[]::text[]) AS nouns,
            COALESCE(k.kv, '{}'::jsonb) AS kv
        FROM bp_objects AS o
        LEFT JOIN LATERAL (
            SELECT ARRAY_AGG(n.noun ORDER BY n.noun) AS nouns
            FROM bp_object_nouns AS n
            WHERE n.obj_id = o.id
        ) AS n ON true
        LEFT JOIN LATERAL (
            SELECT JSONB_OBJECT_AGG(k.key, k.value) AS kv
            FROM bp_objects_kv AS k
            WHERE k.object_id = o.id
        ) AS k ON true
        WHERE o.room_id = $1
        ORDER BY COALESCE(o.position, 0), o.name
        "#,
                &[&room_id],
            )
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let obj = BlueprintObject::try_from_row(&row)?;
            out.push(obj);
        }
        Ok(out)
    }

    async fn room_scripts(&self, room_id: RoomId) -> DbResult<RoomScripts> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                &format!("SELECT hook, script FROM bp_room_scripts WHERE room_id = $1"),
                &[&room_id],
            )
            .await?;

        let mut scripts = RoomScripts::new();
        for row in &rows {
            let hook_str = row.get::<_, String>(0);
            let hook = ScriptHook::from_str(&hook_str)
                .map_err(|_| DbError::Decode(format!("Invalid script hook: {}", hook_str)))?;

            scripts.insert(hook, row.get::<_, String>(1));
        }

        Ok(scripts)
    }

    async fn room_kv(&self, room_id: RoomId) -> DbResult<Kv> {
        let client = self.db.get_client().await?;

        let rows = client
            .query(
                r#"
            SELECT key, value
            FROM bp_room_kv
            WHERE room_id = $1
            "#,
                &[&room_id],
            )
            .await?;

        Ok(Kv::try_from_rows(&rows).map_err(|_| DbError::Decode("Cannot decode row to kv".into()))?)
    }

    async fn set_entry(&self, key: &BlueprintAndRoomKey) -> DbResult<bool> {
        let c = self.db.get_client().await?;

        let n = c
            .execute(
                r#"
                UPDATE blueprints AS b
                    SET entry_room_id = r.id
                FROM bp_rooms AS r
                    WHERE b.key = $1 AND r.key = $2 AND r.bp_id = b.id
            "#,
                &[&key.bp_key, &key.room_key],
            )
            .await?;

        Ok(n == 1)
    }

    async fn add_exit(
        &self,
        from_key: &BlueprintAndRoomKey,
        dir: &str,
        to_key: &BlueprintAndRoomKey,
    ) -> DbResult<bool> {
        if from_key.bp_key != to_key.bp_key {
            return Err(DbError::Validation("from/to must be in the same blueprint".into()));
        }

        let dir = dir.to_lowercase();

        let c = self.db.get_client().await?;
        let n = c
            .execute(
                r#"
            INSERT INTO bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
            SELECT fr.id, $3, tr.id, false, '', false
            FROM bp_rooms AS fr
            JOIN blueprints AS bp ON bp.id = fr.bp_id
            JOIN bp_rooms AS tr ON tr.bp_id = fr.bp_id AND tr.key = $4
            WHERE bp.key = $1 AND fr.key = $2
            ON CONFLICT (from_room_id, dir)
            DO UPDATE SET to_room_id = EXCLUDED.to_room_id
            "#,
                &[&from_key.bp_key, &from_key.room_key, &dir, &to_key.room_key],
            )
            .await?;

        Ok(n == 1)
    }

    async fn set_locked(&self, key: &BlueprintAndRoomKey, locked: bool) -> DbResult<bool> {
        let c = self.db.get_client().await?;

        let n = c
            .execute(
                r#"
                UPDATE bp_rooms AS r
                    SET locked = $3
                FROM blueprints AS bp
                WHERE bp.id = r.bp_id AND bp.key = $1 AND r.key = $2
            "#,
                &[&key.bp_key, &key.room_key, &locked],
            )
            .await?;

        Ok(n == 1)
    }

    async fn insert_blueprint(&self, bp_key: &str, title: &str, account_id: AccountId) -> DbResult<bool> {
        let c = self.db.get_client().await?;

        let n = c
            .execute(
                r#"
            INSERT INTO blueprints (key, title, owner_id, status)
            VALUES ($1, $2, $3, 'draft')
            ON CONFLICT (key) DO NOTHING
            "#,
                &[&bp_key, &title, &account_id],
            )
            .await?;

        Ok(n == 1)
    }

    async fn insert_room(&self, key: &BlueprintAndRoomKey, title: &str, body: &str) -> DbResult<bool> {
        let c = self.db.get_client().await?;

        // Insert only if the blueprint exists; ignore if (bp_id, key) already exists.
        let n = c
            .execute(
                r#"
            INSERT INTO bp_rooms (bp_id, key, title, body, locked, short, hints, scripts)
            SELECT b.id, $2, $3, $4,
                false,
                ''::text,
                ARRAY[]::text[],
                '{}'::jsonb
            FROM blueprints AS b
            WHERE b.key = $1
            ON CONFLICT (bp_id, key) DO NOTHING
            "#,
                &[&key.bp_key, &key.room_key, &title, &body],
            )
            .await?;

        Ok(n == 1)
    }

    async fn submit(&self, bp_key: &str) -> DbResult<bool> {
        let c = self.db.get_client().await?;

        let n = c
            .execute(
                r#"
            UPDATE blueprints
            SET status = 'pending'
            WHERE key = $1 AND status = 'draft'
            "#,
                &[&bp_key],
            )
            .await?;

        Ok(n == 1)
    }
}
