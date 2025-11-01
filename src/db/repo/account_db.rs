use crate::db::repo::account::AccountRepo;
use crate::db::{Db, DbResult, map_row_opt};
use crate::models::account::Account;
use crate::models::types::AccountId;
use std::sync::Arc;

pub struct AccountRepository {
    db: Arc<Db>,
}

impl AccountRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db: db.clone() }
    }
}

#[async_trait::async_trait]
impl AccountRepo for AccountRepository {
    async fn get_by_username(&self, username: &str) -> DbResult<Option<Account>> {
        let client = self.db.get_client().await?;

        let stmt = client
            .prepare_cached(" SELECT * FROM accounts WHERE username = $1 ")
            .await?;

        let row_opt = client.query_opt(&stmt, &[&username]).await?;
        map_row_opt(
            row_opt,
            Account::try_from_row,
            &format!("AccountRepo::get_by_username username={}", username),
        )
    }

    async fn get_by_email(&self, email: &str) -> DbResult<Option<Account>> {
        let client = self.db.get_client().await?;

        let stmt = client
            .prepare_cached(" SELECT * FROM accounts WHERE email = $1 ")
            .await?;

        let row_opt = client.query_opt(&stmt, &[&email]).await?;
        map_row_opt(
            row_opt,
            Account::try_from_row,
            &format!("AccountRepo::get_by_email email={}", email),
        )
    }

    async fn get_by_id(&self, account_id: AccountId) -> DbResult<Option<Account>> {
        let client = self.db.get_client().await?;

        let stmt = client.prepare_cached("SELECT * FROM accounts WHERE id = $1").await?;

        let row_opt = client.query_opt(&stmt, &[&account_id]).await?;
        map_row_opt(
            row_opt,
            Account::try_from_row,
            &format!("AccountRepo::get_by_id id={}", account_id),
        )
    }

    async fn insert_account(&self, account: Account) -> DbResult<Account> {
        let client = self.db.get_client().await?;

        let stmt = client.prepare_cached(
            r#"
            INSERT INTO accounts (username, email, password_hash, role, current_realm_id, current_room_id, xp, health, coins, inventory, flags)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, username, role, created_at, last_login,
                current_realm_id, current_room_id, xp, health, coins,
                inventory, flags
            "#,
        ).await?;

        let row = client
            .query_one(
                &stmt,
                &[
                    &account.username,
                    &account.email,
                    &account.password_hash,
                    &account.role,
                    // &account.zone_id,
                    // &account.current_room_id,
                    // &(account.xp as i64),
                    // &(account.health as i64),
                    // &(account.coins as i64),
                    // &serde_json::to_value(&account.inventory)?,
                    // &serde_json::to_value(&account.flags)?,
                ],
            )
            .await?;

        Account::try_from_row(&row)
    }

    async fn update_last_login(&self, id: AccountId) -> DbResult<()> {
        let client = self.db.get_client().await?;

        let stmt = client
            .prepare_cached("UPDATE accounts SET last_login = NOW() WHERE id = $1")
            .await?;
        client.execute(&stmt, &[&id]).await?;

        Ok(())
    }
}
