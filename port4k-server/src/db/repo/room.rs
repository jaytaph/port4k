use anyhow::Result;
use uuid::Uuid;
use std::collections::HashMap;
use deadpool_postgres::Pool;
use crate::db::models::room::{BlueprintRoom, RoomExitRow, RoomKv, RoomObject, RoomScripts, RoomView, ZoneRoomState};
use crate::db::types::{BlueprintId, Direction, RoomId, ScriptSource, ZoneId};

pub struct RoomRepository<'a> {
    pub pool: &'a Pool,
}

impl<'a> RoomRepository<'a> {
    /// from fn: `RoomRepository::fetch_blueprint_room`
    pub async fn fetch_blueprint_room(&self, room_id: RoomId) -> Result<BlueprintRoom> {
        let client = self.pool.get().await?;

        let row = client.query_one(
            r#"
            SELECT r.id, r.bp_id, r.key, r.title, r.body, r.lockdown, r.short, r.hints, r.scripts
            FROM bp_rooms r
            WHERE r.id = $1
            "#,
            &[&room_id.0],
        ).await?;

        Ok(BlueprintRoom {
            id: RoomId(row.get::<_, Uuid>(0)),
            bp_id: BlueprintId(row.get::<_, Uuid>(1)),
            key: row.get::<_, String>(2),
            title: row.get::<_, String>(3),
            body: row.get::<_, String>(4),
            lockdown: row.get::<_, bool>(5),
            short: row.get::<_, Option<String>>(6),
            hints: row.get::<_, Option<serde_json::Value>>(7),
            scripts_inline: row.get::<_, Option<serde_json::Value>>(8),
        })
    }

    /// from fn: `RoomRepository::fetch_exits`
    pub async fn fetch_exits(&self, room_id: RoomId) -> Result<Vec<RoomExitRow>> {
        let client = self.pool.get().await?;

        let rows = client.query(
            r#"
            SELECT from_room_id, dir, to_room_id, locked, description, visible_when_locked
            FROM bp_exits
            WHERE from_room_id = $1
            ORDER BY dir
            "#,
            &[&room_id.0],
        ).await?;

        let exits = rows.into_iter().map(|r| RoomExitRow {
            from_room_id: RoomId(r.get::<_, Uuid>(0)),
            dir: Direction::from(r.get::<_, String>(1)),
            to_room_id: RoomId(r.get::<_, Uuid>(2)),
            locked: r.get::<_, bool>(3),
            description: r.get::<_, Option<String>>(4),
            visible_when_locked: r.get::<_, bool>(5),
        }).collect();

        Ok(exits)
    }

    /// from fn: `RoomRepository::fetch_objects_with_nouns`
    pub async fn fetch_objects_with_nouns(&self, room_id: RoomId) -> Result<Vec<RoomObject>> {
        let client = self.pool.get().await?;

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
                id,
                name: r.get(2),
                short: r.get(3),
                description: r.get(4),
                examine: r.get(5),
                state: r.get(6),
                use_lua: r.get(7),
                position: r.get(8),
                nouns: nouns_by_obj.remove(&id).unwrap_or_default(),
            }
        }).collect();

        Ok(objects)
    }

    /// from fn: `RoomRepository::fetch_scripts`
    pub async fn fetch_scripts(&self, room_id: RoomId, src: ScriptSource) -> Result<RoomScripts> {
        let client = self.pool.get().await?;

        let (table, enter_col, cmd_col) = match src {
            ScriptSource::Live  => ("bp_room_scripts", "on_enter_lua", "on_command_lua"),
            ScriptSource::Draft => {
                // Draft is per-event; fetch both rows and merge.
                // We’ll do a small UNION query to return both in one pass.
                return self.fetch_draft_scripts(room_id).await;
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

    /// from fn: `RoomRepository::fetch_draft_scripts`
    async fn fetch_draft_scripts(&self, room_id: RoomId) -> Result<RoomScripts> {
        let client = self.pool.get().await?;

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

    /// from fn: `RoomRepository::fetch_room_kv`
    pub async fn fetch_room_kv(&self, room_id: RoomId) -> Result<RoomKv> {
        let client = self.pool.get().await?;

        let rows = client.query(
            r#"
            SELECT key, value
            FROM bp_room_kv
            WHERE room_id = $1
            "#,
            &[&room_id.0],
        ).await?;

        let mut kv = HashMap::new();
        for r in rows {
            let k: String = r.get(0);
            let v: serde_json::Value = r.get(1);
            kv.insert(k, v);
        }
        Ok(kv)
    }

    /// from fn: `RoomRepository::fetch_zone_state`
    pub async fn fetch_zone_state(&self, zone_id: ZoneId, room_id: RoomId) -> Result<Option<ZoneRoomState>> {
        let client = self.pool.get().await?;

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

    /// from fn: `RoomRepository::fetch_view`
    ///
    /// Loads a fully-resolved `RoomView` for engine usage. If `zone_id` is `None`,
    /// you get a playtest-like view (no zone-scoped state).
    pub async fn fetch_view(
        &self,
        room_id: RoomId,
        zone_id: Option<ZoneId>,
        scripts: ScriptSource,
    ) -> Result<RoomView> {
        let room = self.fetch_blueprint_room(room_id).await?;
        let (exits, objects, scripts, room_kv, zone_state) = tokio::try_join!(
            self.fetch_exits(room_id),
            self.fetch_objects_with_nouns(room_id),
            self.fetch_scripts(room_id, scripts),
            self.fetch_room_kv(room_id),
            async {
                if let Some(z) = zone_id {
                    self.fetch_zone_state(z, room_id).await
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
}
