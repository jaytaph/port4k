use uuid::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use crate::db::{Db, DbResult};
use crate::models::blueprint::Blueprint;
use crate::models::room::{BlueprintRoom, RoomExitRow, RoomKv, RoomObject, RoomScripts, RoomView, ZoneRoomState};
use crate::db::repo::room::RoomRepo;
use crate::models::types::{ObjectId, RoomId, ScriptSource, ZoneId};
use crate::error::AppResult;

pub struct RoomRepository {
    pub db: Arc<Db>,
}

impl RoomRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db: db.clone() }
    }

    async fn get_draft_scripts(&self, room_id: RoomId) -> DbResult<RoomScripts> {
        let client = self.db.get_client().await?;

        let rows = client.query(
            r#"
            SELECT event, source
            FROM bp_scripts_draft
            WHERE room_id = $1 AND event IN ('on_enter', 'on_command')
            "#,
            &[&room_id.0],
        ).await?;

        let mut s = RoomScripts::default();
        for r in rows {
            let event: String = r.get(0);
            let source: String = r.get(1);
            match event.as_str() {
                "on_enter"   => s.on_enter_lua = Some(source),
                "on_command" => s.on_command_lua = Some(source),
                _ => {}
            }
        }
        Ok(s)
    }
}

#[async_trait::async_trait]
impl RoomRepo for RoomRepository {
    async fn get_blueprint(&self, bp_key: &str) -> DbResult<Blueprint> {
        let client = self.db.get_client().await?;

        let row = client.query_one(
            r#"
            SELECT id, key, title, owner, entry_room_key, status, created_at, updated_at
            FROM blueprints
            WHERE key = $1
            "#,
            &[&bp_key],
        ).await?;

        Ok(Blueprint::from_row(row))
    }

    async fn get_blueprint_room(&self, room_id: RoomId) -> AppResult<BlueprintRoom> {
        let client = self.db.get_client().await?;

        let row = client.query_one(
            r#"
            SELECT r.id, r.bp_id, r.key, r.title, r.body, r.lockdown, r.short, r.hints, r.scripts
            FROM bp_rooms r
            WHERE r.id = $1
            "#,
            &[&room_id.0],
        ).await?;

        Ok(BlueprintRoom::from_row(row))
    }

    async fn get_exits(&self, room_id: RoomId) -> AppResult<Vec<RoomExitRow>> {
        let client = self.db.get_client().await?;

        let rows = client.query(
            r#"
            SELECT from_room_id, dir, to_room_id, locked, description, visible_when_locked
            FROM bp_exits
            WHERE from_room_id = $1
            ORDER BY dir
            "#,
            &[&room_id.0],
        ).await?;

        Ok(rows.into_iter().map(RoomExitRow::from_row).collect())
    }

    async fn get_objects_with_nouns(&self, room_id: RoomId) -> AppResult<Vec<RoomObject>> {
        let client = self.db.get_client().await?;

        let obj_rows = client.query(
            r#"
            SELECT id, room_id, name, short, description, examine, state, use_lua, position
            FROM bp_objects
            WHERE room_id = $1
            ORDER BY COALESCE(position, 0), name
            "#,
            &[&room_id.0],
        ).await?;

        // Gather nouns in one go
        let noun_rows = client.query(
            r#"
            SELECT room_id, obj_id, noun
            FROM bp_object_nouns
            WHERE room_id = $1
            "#,
            &[&room_id.0],
        ).await?;

        let mut nouns_by_obj: HashMap<Uuid, Vec<String>> = HashMap::new();
        for r in noun_rows {
            let obj_id: Uuid = r.get(1);
            let noun: String = r.get(2);
            nouns_by_obj.entry(obj_id).or_default().push(noun);
        }

        let objects = obj_rows.into_iter().map(|r| {
            let id: Uuid = r.get(0);
            RoomObject {
                id: ObjectId::from_uuid(id),
                name: r.get(2),
                short: r.get(3),
                description: r.get(4),
                examine: r.get(5),
                state: r.get(6),
                use_lua: r.get(7),
                position: r.get(8),
                nouns: nouns_by_obj.remove(&id).unwrap_or_default(),
                initial_qty: None,
                qty: None,
                locked: false,
                revealed: false,
                takeable: false,
                stackable: false,
                is_coin: false,
            }
        }).collect();

        Ok(objects)
    }

    async fn get_scripts(&self, room_id: RoomId, src: ScriptSource) -> AppResult<RoomScripts> {
        let client = self.db.get_client().await?;

        let (table, enter_col, cmd_col) = match src {
            ScriptSource::Live  => ("bp_room_scripts", "on_enter_lua", "on_command_lua"),
            ScriptSource::Draft => {
                // Draft is per-event; fetch both rows and merge.
                // We’ll do a small UNION query to return both in one pass.
                return self.get_draft_scripts(room_id).await;
            }
        };

        let row_opt = client.query_opt(
            &format!("SELECT {enter}, {cmd} FROM {table} WHERE room_id = $1",
                     enter = enter_col, cmd = cmd_col, table = table),
            &[&room_id.0],
        ).await?;

        if let Some(r) = row_opt {
            Ok(RoomScripts {
                on_enter_lua: r.get::<_, Option<String>>(0),
                on_command_lua: r.get::<_, Option<String>>(1),
            })
        } else {
            Ok(RoomScripts::default())
        }
    }

    async fn get_room_kv(&self, room_id: RoomId) -> AppResult<RoomKv> {
        let client = self.db.get_client().await?;

        let rows = client.query(
            r#"
            SELECT key, value
            FROM bp_room_kv
            WHERE room_id = $1
            "#,
            &[&room_id.0],
        ).await?;

        let mut kv: RoomKv = HashMap::new();
        for r in rows {
            let k: String = r.get(0);
            let v: serde_json::Value = r.get(1);

            // Expect value to be a JSON array of strings
            let vec = v
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("expected array for key {}", k))?
                .iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>();

            kv.insert(k, vec);
        }

        Ok(kv)
    }

    async fn get_zone_state(&self, zone_id: ZoneId, room_id: RoomId) -> AppResult<Option<ZoneRoomState>> {
        let client = self.db.get_client().await?;

        let row = client.query_opt(
            r#"
            SELECT zone_id, room_id, state
            FROM zone_room_state
            WHERE zone_id = $1 AND room_id = $2
            "#,
            &[&zone_id.0, &room_id.0],
        ).await?;

        Ok(row.map(|r| ZoneRoomState {
            zone_id: ZoneId(r.get(0)),
            room_id: RoomId(r.get(1)),
            state: r.get(2),
        }))
    }

    async fn get_view(
        &self,
        room_id: RoomId,
        zone_id: Option<ZoneId>,
        scripts: ScriptSource,
    ) -> AppResult<RoomView> {
        let room = self.get_blueprint_room(room_id).await?;
        let (exits, objects, scripts, room_kv, zone_state) = tokio::try_join!(
            self.get_exits(room_id),
            self.get_objects_with_nouns(room_id),
            self.get_scripts(room_id, scripts),
            self.get_room_kv(room_id),
            async {
                if let Some(z) = zone_id {
                    self.get_zone_state(z, room_id).await
                } else {
                    Ok(None)
                }
            }
        )?;

        Ok(RoomView {
            room,
            exits,
            objects,
            scripts,
            room_kv,
            zone_state,
        })
    }


    async fn set_entry(&self, bp_key: &str, room_key: &str) -> AppResult<bool> {
        let c = self.db.get_client().await?;

        let n = c.execute(
        "UPDATE blueprints SET entry_room_key = $2 WHERE key = $1",
            &[&bp_key, &room_key],
        ).await?;

        Ok(n == 1)
    }

    async fn add_exit(&self, bp_key: &str, from_key: &str, dir: &str, to_key: &str) -> AppResult<bool> {
        let c = self.db.get_client().await?;

        let n = c.execute(
            r#"
            INSERT INTO bp_exits (from_room_id, dir, to_room_id, locked, description, visible_when_locked)
            SELECT fr.id, $3, tr.id, false, '', false
            FROM bp_rooms fr
            JOIN bp_rooms tr ON tr.bp_id = fr.bp_id AND tr.key = $4
            JOIN blueprints bp ON bp.id = fr.bp_id
            WHERE bp.key = $1 AND fr.key = $2
            ON CONFLICT (from_room_id, dir) DO UPDATE SET to_room_id = EXCLUDED.to_room_id
            "#,
            &[&bp_key, &from_key, &dir, &to_key],
        ).await?;

        Ok(n == 1)
    }
    async fn set_locked(&self, bp_key: &str, room_key: &str, locked: bool) -> AppResult<bool> {
        let c = self.db.get_client().await?;

        let n = c.execute(
            r#"
            UPDATE bp_rooms
            SET lockdown = $3
            FROM blueprints bp
            WHERE bp.id = bp_rooms.bp_id AND bp.key = $1 AND bp_rooms.key = $2
            "#,
            &[&bp_key, &room_key, &locked],
        ).await?;

        Ok(n == 1)
    }
    async fn insert_blueprint(&self, bp_key: &str, title: &str, owner: &str) -> AppResult<bool> {
        let c = self.db.get_client().await?;

        let n = c.execute(
            r#"
            INSERT INTO blueprints (key, title, owner, status)
            VALUES ($1, $2, $3, 'draft')
            ON CONFLICT (key) DO NOTHING
            "#,
            &[&bp_key, &title, &owner],
        ).await?;

        Ok(n == 1)
    }
    async fn insert_room(&self, bp_key: &str, room_key: &str, title: &str, body: &str) -> AppResult<bool> {
        let c = self.db.get_client().await?;

        let n = c.execute(
            r#"
            INSERT INTO bp_rooms (bp_id, key, title, body, lockdown, short, hints, scripts)
            SELECT id, $2, $3, $4, false, '', '', '{}'
            FROM blueprints
            WHERE key = $1
            ON CONFLICT (bp_id, key) DO NOTHING
            "#,
            &[&bp_key, &room_key, &title, &body],
        ).await?;

        Ok(n == 1)
    }
    async fn submit(&self, bp_key: &str) -> AppResult<bool> {
        let c = self.db.get_client().await?;

        let n = c.execute(
            r#"
            UPDATE blueprints
            SET status = 'pending'
            WHERE key = $1 AND status = 'draft'
            "#,
            &[&bp_key],
        ).await?;

        Ok(n == 1)
    }
}
