use crate::db::repo::account::AccountRepo;
use crate::error::{AppResult, DomainError};
use crate::models::account::Account;
use crate::models::types::AccountId;
use argon2::Argon2;
use password_hash::rand_core::OsRng;
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use std::sync::Arc;

pub struct AuthService {
    repo: Arc<dyn AccountRepo>,
    argon: Argon2<'static>,
}

impl AuthService {
    pub fn new(repo: Arc<dyn AccountRepo>) -> Self {
        let argon = Argon2::default();
        Self { repo, argon }
    }

    pub async fn register(&self, username: &str, email: &str, password: &str) -> AppResult<bool> {
        if self.repo.get_by_username(username).await?.is_some() {
            return Ok(false);
        }

        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon
            .hash_password(password.as_bytes(), &salt)
            .map_err(DomainError::Password)?
            .to_string();

        let account = Account {
            id: AccountId::new(),
            email: email.to_string(),
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

    pub async fn authenticate(&self, username: &str, password: &str) -> AppResult<Account> {
        let Some(account) = self.repo.get_by_username(username).await? else {
            return Err(DomainError::NotFound);
        };

        let parsed = PasswordHash::new(&account.password_hash).map_err(DomainError::Password)?;
        self.argon.verify_password(password.as_bytes(), &parsed)?;

        Ok(account)
    }

    pub async fn update_last_login(&self, account_id: AccountId) -> AppResult<()> {
        self.repo.update_last_login(account_id).await?;
        Ok(())
    }
}
