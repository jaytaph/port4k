use std::str::FromStr;
use anyhow::anyhow;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::{PasswordHash, SaltString};
use rand_core::OsRng;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

#[derive(Clone, Debug)]
pub struct Db {
    pub(crate) pool: Pool,
}

impl Db {
    pub fn new(url: &str) -> anyhow::Result<Self> {
        let cfg = tokio_postgres::Config::from_str(url)?;
        let mgr = Manager::from_config(cfg, NoTls, ManagerConfig { recycling_method: RecyclingMethod::Fast });
        let pool = Pool::builder(mgr).max_size(16).runtime(Runtime::Tokio1).build().expect("build pool");
        Ok(Self { pool })
    }

    /// Run embedded SQL migrations (idempotent).
    pub async fn init(&self) -> anyhow::Result<()> {
        let mut client = self.pool.get().await?;
        embedded::migrations::runner()
            .run_async(&mut **client)
            .await?;
        Ok(())
    }

    pub async fn user_exists(&self, name: &str) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
            "SELECT 1 FROM accounts WHERE username = $1",
                &[&name],
                )
            .await?;
        Ok(row.is_some())
    }

    /// Create a new user with Argon2id password hash. Returns false if name exists.
    pub async fn register_user(&self, name: &str, password: &str) -> anyhow::Result<bool> {
        // check existence
        if self.user_exists(name).await? {
            return Ok(false);
        }

        // hash password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(password.as_bytes(), &salt).map_err(|e| anyhow!(e))?.to_string();

        // insert
        let client = self.pool.get().await?;
        let n = client
            .execute(
                "INSERT INTO accounts (username, role, password_hash) VALUES ($1, 'player', $2)",
                &[&name, &hash],
            )
            .await?;
        Ok(n == 1)
    }

    /// Verify username/password.
    pub async fn verify_user(&self, name: &str, password: &str) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT password_hash FROM accounts WHERE username = $1",
                &[&name],
            )
            .await?;
        let Some(row) = row else { return Ok(false); };
        let Some(stored): Option<String> = row.try_get(0).ok() else { return Ok(false); };

        // empty or NULL means not set
        if stored.trim().is_empty() {
            return Ok(false);
        }

        let parsed = PasswordHash::new(&stored).map_err(|e| anyhow!(e))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    pub async fn start_room_id(&self) -> anyhow::Result<i64> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "SELECT r.id
            FROM rooms r
            JOIN zones z ON z.id = r.zone_id
            WHERE z.key = 'start' AND r.key = 'entry'",
            &[],
        ).await?;
        Ok(row.get::<_, i64>(0))
    }

    pub async fn get_or_create_character(&self, account: &str) -> anyhow::Result<(i64, i64)> {
        let client = self.pool.get().await?;
        if let Some(row) = client.query_opt(
            "SELECT id, location_id FROM characters WHERE account_name = $1 ORDER BY id LIMIT 1",
            &[&account],
        ).await? {
            let id: i64 = row.get(0);
            let loc: Option<i64> = row.get(1);
            let loc = if let Some(l) = loc { l } else {
                let s = self.start_room_id().await?;
                client.execute("UPDATE characters SET location_id=$1 WHERE id=$2", &[&s, &id]).await?;
                s
            };
            return Ok((id, loc));
        }
        let loc = self.start_room_id().await?;
        let name = account; // simple default; one char per account for now
        let row = client.query_one(
            "INSERT INTO characters (account_name, name, location_id)
            VALUES ($1, $2, $3)
            RETURNING id, location_id",
            &[&account, &name, &loc],
        ).await?;
        Ok((row.get(0), row.get(1)))
    }

    pub async fn room_view(&self, room_id: i64) -> anyhow::Result<String> {
        let client = self.pool.get().await?;
        let r = client.query_one("SELECT title, body FROM rooms WHERE id=$1", &[&room_id]).await?;
        let title: String = r.get(0);
        let body: String = r.get(1);
        let exits = client.query(
            "SELECT dir FROM exits WHERE from_room=$1 ORDER BY dir",
            &[&room_id],
        ).await?;
        let dirs: Vec<String> = exits.into_iter().map(|row| row.get::<_, String>(0)).collect();
        let exits_line = if dirs.is_empty() { "Exits: none".to_string() } else { format!("Exits: {}", dirs.join(", ")) };
        Ok(format!("{}\n{}\n{}\n", title, body, exits_line))
    }

    pub async fn move_character(&self, account: &str, dir: &str) -> anyhow::Result<Option<i64>> {
        let client = self.pool.get().await?;
        // find current room
        let row = client.query_opt(
            "SELECT c.id, c.location_id FROM characters c WHERE c.account_name=$1 ORDER BY c.id LIMIT 1",
            &[&account],
        ).await?;
        let Some(row) = row else { return Ok(None); };
        let cid: i64 = row.get(0);
        let cur: i64 = row.get(1);
        // follow exit
        let to = client.query_opt(
            "SELECT to_room FROM exits WHERE from_room=$1 AND dir=LOWER($2)",
            &[&cur, &dir],
        ).await?;
        let Some(to_row) = to else { return Ok(None); };
        let new_room: i64 = to_row.get(0);
        client.execute("UPDATE characters SET location_id=$1 WHERE id=$2", &[&new_room, &cid]).await?;
        Ok(Some(new_room))
    }

    /// Spawn due coin piles (and other loot) up to max_instances per spawn.
    /// Returns number of piles spawned.
    pub async fn spawn_tick(&self) -> anyhow::Result<u64> {
        use rand_core::RngCore;
        let mut spawned = 0u64;
        let mut client = self.pool.get().await?;
        let tx = client.build_transaction().start().await?;
        // Lock due spawns so multiple workers don't collide
        let rows = tx.query(
            "SELECT id, room_id, item, qty_min, qty_max, interval_ms, max_instances
            FROM loot_spawns
            WHERE next_spawn_at <= now()
            sFOR UPDATE SKIP LOCKED",
            &[],
        ).await?;
        for row in rows {
            let spawn_id: i64 = row.get(0);
            let room_id: i64 = row.get(1);
            let item: String = row.get(2);
            let qty_min: i32 = row.get(3);
            let qty_max: i32 = row.get(4);
            let interval_ms: i32 = row.get(5);
            let max_instances: i32 = row.get(6);

            let cur_count: i64 = tx.query_one(
                "SELECT COUNT(*) FROM room_loot
              WHERE room_id = $1 AND item = $2 AND picked_by IS NULL",
                &[&room_id, &item]
            ).await?.get(0);

            if cur_count < max_instances as i64 {
                // random qty in [qty_min, qty_max]
                let mut rng = OsRng;
                let span = (qty_max - qty_min + 1).max(1) as u32;
                let r = (rng.next_u32() % span) as i32;
                let qty = qty_min + r;

                tx.execute(
                    "INSERT INTO room_loot (room_id, item, qty) VALUES ($1, $2, $3)",
                    &[&room_id, &item, &qty]
                ).await?;
                spawned += 1;
            }

            tx.execute(
                "UPDATE loot_spawns SET next_spawn_at = now() + make_interval(secs := $1::int / 1000.0)
                WHERE id = $2",
                &[&interval_ms, &spawn_id]
            ).await?;
        }
        tx.commit().await?;
        Ok(spawned)
    }

    /// Return total visible coins in a room (sum of available piles).
    pub async fn room_coin_total(&self, room_id: i64) -> anyhow::Result<i64> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "SELECT COALESCE(SUM(qty), 0)
            FROM room_loot
            WHERE room_id = $1 AND item = 'coin' AND picked_by IS NULL",
            &[&room_id]
        ).await?;
        Ok(row.get(0))
    }

    /// Atomically pick up to `want_qty` coins from the room. Returns actually picked.
    pub async fn pickup_coins(&self, account: &str, room_id: i64, want_qty: i32) -> anyhow::Result<i32> {
        let mut client = self.pool.get().await?;
        let tx = client.build_transaction().start().await?;

        // Lock one available pile (largest first) – avoids two players grabbing same pile
        let coin = tx.query_opt(
            "SELECT id, qty
            FROM room_loot
            WHERE room_id = $1 AND item = 'coin' AND picked_by IS NULL
            ORDER BY qty DESC
            FOR UPDATE SKIP LOCKED
            LIMIT 1",
            &[&room_id]
        ).await?;

        let Some(row) = coin else {
            tx.commit().await?;
            return Ok(0);
        };

        let loot_id: i64 = row.get(0);
        let qty: i32 = row.get(1);
        let take = qty.min(want_qty.max(1));

        if qty > take {
            tx.execute("UPDATE room_loot SET qty = qty - $1 WHERE id = $2", &[&take, &loot_id]).await?;
        } else {
            tx.execute(
                "UPDATE room_loot SET picked_by = $1, picked_at = now() WHERE id = $2",
                &[&account, &loot_id]
            ).await?;
        }

        let take64 = i64::from(take);
        tx.execute(
            "UPDATE accounts SET balance = balance + $1 WHERE username = $2",
            &[&take64, &account]
        ).await?;

        tx.commit().await?;
        Ok(take)
    }

    /// Read current account balance.
    pub async fn account_balance(&self, account: &str) -> anyhow::Result<i64> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "SELECT balance FROM accounts WHERE username = $1",
            &[&account]
        ).await?;
        Ok(row.get(0))
    }

    pub async fn bp_new(&self, bp_key: &str, title: &str, owner: &str) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        let n = client.execute(
            "INSERT INTO blueprints (key, title, owner) VALUES ($1, $2, $3)
            ON CONFLICT DO NOTHING",
            &[&bp_key, &title, &owner],
        ).await?;
        Ok(n == 1)
    }

    pub async fn bp_room_add(&self, bp_key: &str, room_key: &str, title: &str, body: &str) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        let n = client.execute(
            "INSERT INTO blueprint_rooms (bp_key, key, title, body) VALUES ($1, $2, $3, $4)
            ON CONFLICT DO NOTHING",
            &[&bp_key, &room_key, &title, &body],
        ).await?;
        Ok(n == 1)
    }

    pub async fn bp_exit_add(&self, bp_key: &str, from_key: &str, dir: &str, to_key: &str) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        let n = client.execute(
            "INSERT INTO blueprint_exits (bp_key, from_key, dir, to_key)
            VALUES ($1, $2, LOWER($3), $4)
            ON CONFLICT DO NOTHING",
            &[&bp_key, &from_key, &dir, &to_key],
        ).await?;
        Ok(n == 1)
    }

    pub async fn bp_set_entry(&self, bp_key: &str, room_key: &str) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        let n = client.execute(
            "UPDATE blueprints SET entry_room_key = $2 WHERE key = $1",
            &[&bp_key, &room_key],
        ).await?;
        Ok(n == 1)
    }

    pub async fn bp_entry(&self, bp_key: &str) -> anyhow::Result<Option<String>> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT entry_room_key FROM blueprints WHERE key=$1",
            &[&bp_key],
        ).await?;
        Ok(row.and_then(|r| r.get::<_, Option<String>>(0)))
    }

    pub async fn bp_room_view(&self, bp_key: &str, room_key: &str) -> anyhow::Result<Option<String>> {
        let client = self.pool.get().await?;
        let r = client.query_opt(
            "SELECT title, body FROM blueprint_rooms WHERE bp_key=$1 AND key=$2",
            &[&bp_key, &room_key],
        ).await?;
        let Some(r) = r else { return Ok(None); };
        let title: String = r.get(0);
        let body: String = r.get(1);
        let exits = client.query(
            "SELECT dir FROM blueprint_exits WHERE bp_key=$1 AND from_key=$2 ORDER BY dir",
            &[&bp_key, &room_key],
        ).await?;
        let dirs: Vec<String> = exits.into_iter().map(|row| row.get::<_, String>(0)).collect();
        let exits_line = if dirs.is_empty() { "Exits: none".to_string() } else { format!("Exits: {}", dirs.join(", ")) };
        Ok(Some(format!("{title}\n{body}\n{exits_line}\n")))
    }

    pub async fn bp_move(&self, bp_key: &str, from_key: &str, dir: &str) -> anyhow::Result<Option<String>> {
        let client = self.pool.get().await?;
        let to = client.query_opt(
            "SELECT to_key FROM blueprint_exits
            WHERE bp_key=$1 AND from_key=$2 AND dir=LOWER($3)",
            &[&bp_key, &from_key, &dir],
        ).await?;
        Ok(to.map(|r| r.get::<_, String>(0)))
    }

    pub async fn bp_script_put_draft(
        &self, bp_key: &str, room_key: &str, event: &str, source: &str, author: &str,
    ) -> anyhow::Result<()> {
        let client = self.pool.get().await?;
        client.execute(
            "INSERT INTO blueprint_scripts_draft (bp_key, room_key, event, source, author)
            VALUES ($1,$2,$3,$4,$5)
            ON CONFLICT (bp_key,room_key,event)
            DO UPDATE SET source=EXCLUDED.source, author=EXCLUDED.author, updated_at=now()",
            &[&bp_key, &room_key, &event, &source, &author],
        ).await?;
        Ok(())
    }

    pub async fn bp_script_publish(
        &self, bp_key: &str, room_key: &str, event: &str,
    ) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        // copy draft → live
        let n = client.execute(
            "INSERT INTO blueprint_scripts_live (bp_key, room_key, event, source, author, updated_at)
            SELECT bp_key, room_key, event, source, author, now()
            FROM blueprint_scripts_draft
            WHERE bp_key=$1 AND room_key=$2 AND event=$3
            ON CONFLICT (bp_key,room_key,event)
            DO UPDATE SET source=EXCLUDED.source, author=EXCLUDED.author, updated_at=now()",
            &[&bp_key, &room_key, &event],
        ).await?;
        Ok(n > 0)
    }

    pub async fn bp_script_get_live(
        &self, bp_key: &str, room_key: &str, event: &str,
    ) -> anyhow::Result<Option<String>> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT source FROM blueprint_scripts_live WHERE bp_key=$1 AND room_key=$2 AND event=$3",
            &[&bp_key, &room_key, &event],
        ).await?;
        Ok(row.map(|r| r.get::<_, String>(0)))
    }

    pub async fn bp_room_kv_get(&self, bp: &str, room: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT value FROM blueprint_room_kv WHERE bp_key=$1 AND room_key=$2 AND key=$3",
            &[&bp, &room, &key]
        ).await?;
        Ok(row.map(|r| r.get::<_, serde_json::Value>(0)))
    }

    pub async fn bp_room_kv_set(&self, bp: &str, room: &str, key: &str, value: &serde_json::Value) -> anyhow::Result<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO blueprint_room_kv (bp_key, room_key, key, value)
            VALUES ($1,$2,$3,$4)
            ON CONFLICT (bp_key,room_key,key)
            DO UPDATE SET value=EXCLUDED.value",
            &[&bp, &room, &key, value]
        ).await?;
        Ok(())
    }

    pub async fn bp_player_kv_get(&self, bp: &str, account: &str, room: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let c = self.pool.get().await?;
        let row = c.query_opt(
            "SELECT value FROM blueprint_player_kv WHERE bp_key=$1 AND account_name=$2 AND room_key=$3 AND key=$4",
            &[&bp, &account, &room, &key]
        ).await?;
        Ok(row.map(|r| r.get::<_, serde_json::Value>(0)))
    }

    pub async fn bp_player_kv_set(&self, bp: &str, account: &str, room: &str, key: &str, value: &serde_json::Value) -> anyhow::Result<()> {
        let c = self.pool.get().await?;
        c.execute(
            "INSERT INTO blueprint_player_kv (bp_key, account_name, room_key, key, value)
            VALUES ($1,$2,$3,$4,$5)
            ON CONFLICT (bp_key,account_name,room_key,key)
            DO UPDATE SET value=EXCLUDED.value",
            &[&bp, &account, &room, &key, value]
        ).await?;
        Ok(())
    }
}