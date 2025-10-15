// use anyhow::anyhow;
// use argon2::{Argon2, PasswordHasher, PasswordVerifier};
// use password_hash::{PasswordHash, SaltString};
// use rand_core::OsRng;
// use super::Db;
//
// impl Db {
// /// Create a new user with Argon2id password hash. Returns false if name exists.
// pub async fn register_user(&self, username: &str, password: &str) -> AppResult<bool> {
//     if self.account_by_username(username).await?.is_some() {
//         return Ok(false);
//     }
//
//     let salt = SaltString::generate(&mut OsRng);
//     let hash = Argon2::default()
//         .hash_password(password.as_bytes(), &salt)
//         .map_err(|e| anyhow!(e))?
//         .to_string();
//
//     let client = self.pool.get().await?;
//     let n = client
//         .execute(
//             "INSERT INTO accounts (username, role, password_hash) VALUES ($1, 'player', $2)",
//             &[&username, &hash],
//         )
//         .await?;
//
//     Ok(n == 1)
// }

// /// Verify username/password.
// pub async fn verify_user(&self, username: &str, password: &str) -> AppResult<bool> {
//     let Some(account) = self.account_by_username(username).await? else {
//         return Ok(false);
//     };
//
//     let parsed = PasswordHash::new(&account.password_hash).map_err(|e| anyhow!(e))?;
//     Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
// }
// }
