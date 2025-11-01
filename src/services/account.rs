use crate::db::repo::AccountRepo;
use crate::error::{AppResult, LoginError};
use crate::models::account::Account;
use crate::models::types::AccountId;
use argon2::Argon2;
use password_hash::{PasswordHash, PasswordVerifier};
use std::sync::Arc;

pub struct AccountService {
    repo: Arc<dyn AccountRepo>,
    argon: Argon2<'static>,
}

pub type LoginResult<T> = Result<T, LoginError>;

impl AccountService {
    pub fn new(repo: Arc<dyn AccountRepo>) -> Self {
        let argon = Argon2::default();
        Self { repo, argon }
    }

    pub async fn get_by_id(&self, account_id: AccountId) -> AppResult<Option<Account>> {
        let account = self.repo.get_by_id(account_id).await?;
        Ok(account)
    }

    pub async fn exists(&self, username: &str) -> AppResult<bool> {
        Ok(self.repo.get_by_username(username).await?.is_some())
    }

    pub async fn login(&self, username: &str, password: &str) -> LoginResult<Account> {
        // Validate username input
        match Account::validate_username(username) {
            Ok(account) => account,
            Err(_) => return Err(LoginError::UserNotFound),
        }

        let Some(account) = self
            .repo
            .get_by_username(username)
            .await
            .map_err(|_| LoginError::UserNotFound)?
        else {
            return Err(LoginError::UserNotFound);
        };

        let parsed = PasswordHash::new(&account.password_hash)
            .map_err(|_| LoginError::InternalError("cannot generate password hash".into()))?;
        if self.argon.verify_password(password.as_bytes(), &parsed).is_err() {
            return Err(LoginError::InvalidPassword);
        };

        if account.locked_out {
            return Err(LoginError::AccountLocked);
        }

        // We are logged in. Update last login time
        self.repo
            .update_last_login(account.id)
            .await
            .map_err(|_| LoginError::InternalError("cannot update login timestamp".into()))?;

        Ok(account)
    }
}
