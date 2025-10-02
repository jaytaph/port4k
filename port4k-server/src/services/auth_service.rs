use anyhow::anyhow;
use argon2::Argon2;
use password_hash::{PasswordHash, PasswordHasher, SaltString};
use rand_core::OsRng;
use crate::db::models::account::Account;
use crate::db::repo::account::AccountRepo;
use crate::db::types::AccountId;

pub struct AuthService<R: AccountRepo> {
    repo: R,
    argon: Argon2<'static>,
}

impl<R: AccountRepo> AuthService<R> {
    pub fn new(repo: R) -> Self {
        let argon = Argon2::default();
        Self { repo, argon }
    }

    pub async fn register(&self, username: &str, password: &str) -> anyhow::Result<bool> {
        if self.repo.get_by_username(username).await?.is_some() {
            return Ok(false);
        }

        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!(e))?
            .to_string();

        let account = Account {
            id: 0, // Will be set by the database
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

        let result = self.repo.insert_account(account).await;
        Ok(result.is_ok())
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> anyhow::Result<bool> {
        let Some(account) = self.repo.get_by_username(username).await? else {
            return Ok(false);
        };

        let parsed = PasswordHash::new(&account.password_hash).map_err(|e| anyhow!(e))?;
        Ok(self.argon.verify_password(password.as_bytes(), &parsed).is_ok())
    }

    pub async fn update_last_login(&self, account_id: AccountId) -> anyhow::Result<()> {
        self.repo.update_last_login(account_id).await
    }
}