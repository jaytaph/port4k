use crate::db::models::account::Account;
use crate::db::types::AccountId;

#[async_trait::async_trait]
pub trait AccountRepo: Send + Sync {
    async fn get_by_username(&self, username: &str) -> anyhow::Result<Option<Account>>;
    async fn get_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>>;
    async fn insert_account(&self, account: Account) -> anyhow::Result<Account>;
    async fn update_last_login(&self, account_id: AccountId) -> anyhow::Result<()>;
}
