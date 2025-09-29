use anyhow::anyhow;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::{PasswordHash, SaltString};
use rand_core::OsRng;

use super::Db;

impl Db {
    pub async fn user_exists(&self, name: &str) -> anyhow::Result<bool> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt("SELECT 1 FROM accounts WHERE username = $1", &[&name])
            .await?;
        Ok(row.is_some())
    }

    /// Create a new user with Argon2id password hash. Returns false if name exists.
    pub async fn register_user(&self, name: &str, password: &str) -> anyhow::Result<bool> {
        if self.user_exists(name).await? {
            return Ok(false);
        }
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!(e))?
            .to_string();

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

        if stored.trim().is_empty() {
            return Ok(false);
        }

        let parsed = PasswordHash::new(&stored).map_err(|e| anyhow!(e))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    /// Read current account balance.
    pub async fn account_balance(&self, account: &str) -> anyhow::Result<i64> {
        let client = self.pool.get().await?;
        let row = client
            .query_one("SELECT balance FROM accounts WHERE username = $1", &[&account])
            .await?;
        Ok(row.get(0))
    }
}
