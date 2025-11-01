// use crate::db::repo::AccountRepo;
// use crate::error::{AppResult, DomainError};
// use crate::models::account::{Account, AccountRole};
// use crate::models::types::AccountId;
// use argon2::Argon2;
// use password_hash::rand_core::OsRng;
// use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
// use std::sync::Arc;
// use tracing::log::warn;
//
// pub struct AuthService {
//     repo: Arc<dyn AccountRepo>,
//     argon: Argon2<'static>,
// }
//
// impl AuthService {
//     pub fn new(repo: Arc<dyn AccountRepo>) -> Self {
//         let argon = Argon2::default();
//         Self { repo, argon }
//     }
//
//     pub async fn register(&self, username: &str, email: &str, password: &str) -> AppResult<bool> {
//         if self.repo.get_by_username(username).await?.is_some() {
//             return Ok(false);
//         }
//
//         let salt = SaltString::generate(&mut OsRng);
//         let hash = self
//             .argon
//             .hash_password(password.as_bytes(), &salt)
//             .map_err(DomainError::Password)?
//             .to_string();
//
//         let account = Account {
//             id: AccountId::new(),
//             email: email.to_string(),
//             username: username.to_string(),
//             role: AccountRole::User,
//             password_hash: hash,
//             last_login: None,
//             created_at: Default::default(),
//         };
//
//         match self.repo.insert_account(account).await {
//             Ok(_) => Ok(true),
//             Err(e) => Err(e.into()),
//         }
//     }
//
//     pub async fn authenticate(&self, username: &str, password: &str) -> AppResult<Account> {
//         let Some(account) = self.repo.get_by_username(username).await? else {
//             warn!(
//                 "[AuthService] Authentication failed for username '{}': not found",
//                 username
//             );
//             return Err(DomainError::NotFound("Account not found".into()));
//         };
//
//         let parsed = PasswordHash::new(&account.password_hash).map_err(DomainError::Password)?;
//         if self.argon.verify_password(password.as_bytes(), &parsed).is_err() {
//             warn!(
//                 "[AuthService] Authentication failed for username '{}': invalid password",
//                 username
//             );
//             return Err(DomainError::NotFound("Account not found".into()));
//         }
//
//         Ok(account)
//     }
//
//     pub async fn update_last_login(&self, account_id: AccountId) -> AppResult<()> {
//         self.repo.update_last_login(account_id).await?;
//         Ok(())
//     }
// }
