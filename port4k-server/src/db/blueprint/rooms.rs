use crate::db::repo::object::ObjectRepo;
use crate::rendering::{render_room, Theme};
use super::super::Db;

impl Db {
    pub async fn bp_room_set_locked(
        &self,
        bp_key: &str,
        room_key: &str,
        locked: bool,
    ) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "UPDATE bp_rooms SET locked=$3
             WHERE bp_key=$1 AND key=$2",
                &[&bp_key, &room_key, &locked],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_room_is_locked(&self, bp_key: &str, room_key: &str) -> anyhow::Result<Option<bool>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT locked FROM bp_rooms WHERE bp_key=$1 AND key=$2",
                &[&bp_key, &room_key],
            )
            .await?;
        Ok(row.and_then(|r| r.get::<_, Option<bool>>(0)))
    }




    pub async fn bp_new(&self, bp_key: &str, title: &str, owner: &str) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "INSERT INTO blueprints (key, title, owner)
             VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
                &[&bp_key, &title, &owner],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_room_add(
        &self,
        bp_key: &str,
        room_key: &str,
        title: &str,
        body: &str,
    ) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "INSERT INTO bp_rooms (bp_key, key, title, body)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT DO NOTHING",
                &[&bp_key, &room_key, &title, &body],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_exit_add(
        &self,
        bp_key: &str,
        from_key: &str,
        dir: &str,
        to_key: &str,
    ) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "INSERT INTO bp_exits (bp_key, from_key, dir, to_key)
             VALUES ($1, $2, LOWER($3), $4)
             ON CONFLICT DO NOTHING",
                &[&bp_key, &from_key, &dir, &to_key],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_set_entry(&self, bp_key: &str, room_key: &str) -> anyhow::Result<bool> {
        let c = self.pool.get().await?;
        let n = c
            .execute(
                "UPDATE blueprints SET entry_room_key = $2 WHERE key = $1",
                &[&bp_key, &room_key],
            )
            .await?;
        Ok(n == 1)
    }

    pub async fn bp_entry(&self, bp_key: &str) -> anyhow::Result<Option<String>> {
        let c = self.pool.get().await?;
        let row = c
            .query_opt(
                "SELECT entry_room_key FROM blueprints WHERE key=$1",
                &[&bp_key],
            )
            .await?;
        Ok(row.and_then(|r| r.get::<_, Option<String>>(0)))
    }

    /// width: wrap column; pass 80 for now (you can later read from per-user setting like "\w 80")
    pub async fn bp_room_view(
        &self,
        bp_key: &str,
        room_key: &str,
        width: usize,
    ) -> anyhow::Result<Option<String>> {
        let c = self.pool.get().await?;

        let row = c
            .query_opt(
                "SELECT title, body FROM bp_rooms WHERE bp_key=$1 AND key=$2",
                &[&bp_key, &room_key],
            )
            .await?;

        let Some(row) = row else { return Ok(None) };

        let title: String = row.get(0);
        let body: String = row.get(1);

        let exits = c
            .query(
                "SELECT dir FROM bp_exits
                    WHERE bp_key=$1 AND from_key=$2
                    ORDER BY dir",
                &[&bp_key, &room_key],
            )
            .await?;
        let exits: Vec<String> = exits.into_iter().map(|r| r.get::<_, String>(0)).collect();

        let objs = ObjectRepo.render_projection(&c, bp_key, room_key).await?;
        let objects: std::collections::HashMap<_, _> =
            objs.into_iter().map(|o| (o.id, o.short)).collect();

        Ok(Some(render_room(&Theme::blue(), &title, &body, &objects, &exits, width)))
    }

    pub async fn bp_move(
        &self,
        bp_key: &str,
        from_key: &str,
        dir: &str,
    ) -> anyhow::Result<Option<String>> {
        let c = self.pool.get().await?;
        let to = c
            .query_opt(
                "SELECT to_key FROM bp_exits
                 WHERE bp_key=$1 AND from_key=$2 AND dir=LOWER($3)",
                &[&bp_key, &from_key, &dir],
            )
            .await?;
        Ok(to.map(|r| r.get::<_, String>(0)))
    }
}
