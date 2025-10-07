use std::sync::Arc;
use argon2::Argon2;
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand_core::OsRng;
use crate::models::account::Account;
use crate::db::repo::account::AccountRepo;
use crate::models::types::AccountId;
use crate::services::ServiceResult;

pub struct AuthService {
    repo: Arc<dyn AccountRepo>,
    argon: Argon2<'static>,
}

impl AuthService {
    pub fn new(repo: Arc<dyn AccountRepo>) -> Self {
        let argon = Argon2::default();
        Self { repo, argon }
    }

    pub async fn register(&self, username: &str, password: &str) -> ServiceResult<bool> {
        if self.repo.get_by_username(username).await?.is_some() {
            return Ok(false);
        }

        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon
            .hash_password(password.as_bytes(), &salt)?
            .to_string();

        let account = Account {
            id: AccountId::new(),
            username: username.to_string(),
            role: "player".to_string(),
            password_hash: hash,
            last_login: None,
            zone_id: None,
            current_room_id: None,
            xp: 0,
            health: 0,
            coins: 0,
            inventory: vec![],
            created_at: Default::default(),
            flags: vec![],
        };

        match self.repo.insert_account(account).await {
            Ok(_) => Ok(true),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> ServiceResult<bool> {
        let Some(account) = self.repo.get_by_username(username).await? else {
            return Ok(false);
        };

        let parsed = PasswordHash::new(&account.password_hash)?;
        Ok(self.argon.verify_password(password.as_bytes(), &parsed).is_ok())
    }

    pub async fn update_last_login(&self, account_id: AccountId) -> ServiceResult<()> {
        self.repo.update_last_login(account_id).await?;
        Ok(())
    }
}