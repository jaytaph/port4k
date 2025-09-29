use rand_core::OsRng;
use super::Db;

impl Db {
    /// Spawn due coin piles (and other loot) up to max_instances per spawn.
    /// Returns number of piles spawned.
    pub async fn spawn_tick(&self) -> anyhow::Result<u64> {
        use rand_core::RngCore;
        let mut spawned = 0u64;
        let mut client = self.pool.get().await?;
        let tx = client.build_transaction().start().await?;

        // NOTE: your original query had a small typo `sFOR` → `FOR`
        let rows = tx
            .query(
                "SELECT id, room_id, item, qty_min, qty_max, interval_ms, max_instances
                 FROM loot_spawns
                 WHERE next_spawn_at <= now()
                 FOR UPDATE SKIP LOCKED",
                &[],
            )
            .await?;

        for row in rows {
            let spawn_id: i64 = row.get(0);
            let room_id: i64 = row.get(1);
            let item: String = row.get(2);
            let qty_min: i32 = row.get(3);
            let qty_max: i32 = row.get(4);
            let interval_ms: i32 = row.get(5);
            let max_instances: i32 = row.get(6);

            let cur_count: i64 = tx
                .query_one(
                    "SELECT COUNT(*)
                     FROM room_loot
                     WHERE room_id = $1 AND item = $2 AND picked_by IS NULL",
                    &[&room_id, &item],
                )
                .await?
                .get(0);

            if cur_count < max_instances as i64 {
                let mut rng = OsRng;
                let span = (qty_max - qty_min + 1).max(1) as u32;
                let r = (rng.next_u32() % span) as i32;
                let qty = qty_min + r;

                tx.execute(
                    "INSERT INTO room_loot (room_id, item, qty) VALUES ($1, $2, $3)",
                    &[&room_id, &item, &qty],
                )
                    .await?;
                spawned += 1;
            }

            tx.execute(
                "UPDATE loot_spawns
                 SET next_spawn_at = now() + make_interval(secs := $1::int / 1000.0)
                 WHERE id = $2",
                &[&interval_ms, &spawn_id],
            )
                .await?;
        }

        tx.commit().await?;
        Ok(spawned)
    }

    /// Atomically pick up to `want_qty` coins from the room. Returns actually picked.
    pub async fn pickup_coins(
        &self,
        account: &str,
        room_id: i64,
        want_qty: i32,
    ) -> anyhow::Result<i32> {
        let mut client = self.pool.get().await?;
        let tx = client.build_transaction().start().await?;

        let coin = tx
            .query_opt(
                "SELECT id, qty
                 FROM room_loot
                 WHERE room_id = $1 AND item = 'coin' AND picked_by IS NULL
                 ORDER BY qty DESC
                 FOR UPDATE SKIP LOCKED
                 LIMIT 1",
                &[&room_id],
            )
            .await?;

        let Some(row) = coin else {
            tx.commit().await?;
            return Ok(0);
        };

        let loot_id: i64 = row.get(0);
        let qty: i32 = row.get(1);
        let take = qty.min(want_qty.max(1));

        if qty > take {
            tx.execute(
                "UPDATE room_loot SET qty = qty - $1 WHERE id = $2",
                &[&take, &loot_id],
            )
                .await?;
        } else {
            tx.execute(
                "UPDATE room_loot SET picked_by = $1, picked_at = now() WHERE id = $2",
                &[&account, &loot_id],
            )
                .await?;
        }

        let take64 = i64::from(take);
        tx.execute(
            "UPDATE accounts SET balance = balance + $1 WHERE username = $2",
            &[&take64, &account],
        )
            .await?;

        tx.commit().await?;
        Ok(take)
    }
}
