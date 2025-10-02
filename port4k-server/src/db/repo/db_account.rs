use deadpool_postgres::Pool;
use crate::db::models::account::Account;
use crate::db::repo::account::AccountRepo;
use crate::db::types::AccountId;

pub struct AccountRepository<'a> {
    pub pool: &'a Pool,
}

#[async_trait::async_trait]
impl<'a> AccountRepo for AccountRepository<'a> {
    async fn get_by_username(&self, username: &str) -> anyhow::Result<Option<Account>> {
        let client = self.pool.get().await?;

        let stmt = client.prepare_cached(
            r#"
            SELECT id, username, role, created_at, last_login,
                zone_id, current_room_id, xp, health, coins,
                inventory, flags
            FROM accounts
            WHERE username = $1
            "#,
        ).await?;

        let row_opt = client.query_opt(&stmt, &[&username]).await?;
        Ok(row_opt.map(Account::from_row))
    }

    async fn get_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>> {
        let client = self.pool.get().await?;

        let stmt = client.prepare_cached(
            r#"
            SELECT id, username, role, created_at, last_login,
                zone_id, current_room_id, xp, health, coins,
                inventory, flags
            FROM accounts
            WHERE id = $1
        "#).await?;

        let row_opt = client.query_opt(&stmt, &[&account_id]).await?;
        Ok(row_opt.map(Account::from_row))

    }

    async fn insert_account(&self, account: Account) -> anyhow::Result<Account> {
        let client = self.pool.get().await?;

        let stmt = client.prepare_cached(
            r#"
            INSERT INTO accounts (username, password_hash, role, zone_id, current_room_id, xp, health, coins, inventory, flags)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, username, role, created_at, last_login,
                zone_id, current_room_id, xp, health, coins,
                inventory, flags
            "#,
        ).await?;

        let row = client.query_one(&stmt, &[
            &account.username,
            &account.password_hash,
            &account.role,
            &account.zone_id,
            &account.current_room_id,
            &(account.xp as i64),
            &(account.health as i64),
            &(account.coins as i64),
            &serde_json::to_value(&account.inventory)?,
            &serde_json::to_value(&account.flags)?,
        ]).await?;

        Ok(Account::from_row(row))

    }

    async fn update_last_login(&self, id: AccountId) -> anyhow::Result<()> {
        let client = self.pool.get().await?;

        let stmt = client.prepare_cached(
            "UPDATE accounts SET last_login = NOW() WHERE id = $1"
        ).await?;
        client.execute(&stmt, &[&id]).await?;

        Ok(())
    }

}
