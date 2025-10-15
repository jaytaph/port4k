use crate::db::repo::account::AccountRepo;
use crate::error::AppResult;
use std::sync::Arc;

pub struct AccountService {
    repo: Arc<dyn AccountRepo>,
}

impl AccountService {
    pub fn new(repo: Arc<dyn AccountRepo>) -> Self {
        Self { repo }
    }

    pub async fn exists(&self, username: &str) -> AppResult<bool> {
        Ok(self.repo.get_by_username(username).await?.is_some())
    }
}
