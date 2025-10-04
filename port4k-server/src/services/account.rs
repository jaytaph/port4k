use std::sync::Arc;
use crate::db::repo::account::AccountRepo;

pub struct AccountService {
    repo: Arc<dyn AccountRepo>,
}

impl AccountService {
    pub fn new(repo: Arc<dyn AccountRepo>) -> Self {
        Self { repo }
    }

    pub async fn exists(&self, username: &str) -> anyhow::Result<bool> {
        Ok(self.repo.get_by_username(username).await?.is_some())
    }
}