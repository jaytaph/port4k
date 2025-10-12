use crate::db::DbResult;
use crate::models::account::Account;
use crate::models::types::AccountId;

#[async_trait::async_trait]
pub trait AccountRepo: Send + Sync {
    async fn get_by_username(&self, username: &str) -> DbResult<Option<Account>>;
    async fn get_by_id(&self, account_id: AccountId) -> DbResult<Option<Account>>;
    async fn insert_account(&self, account: Account) -> DbResult<Account>;
    async fn update_last_login(&self, account_id: AccountId) -> DbResult<()>;
}
