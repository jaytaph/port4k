mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

use super::{Db, DbResult};

impl Db {
    /// Run embedded SQL migrations (idempotent).
    pub async fn init(&self) -> DbResult<()> {
        let mut client = self.pool.get().await?;
        embedded::migrations::runner().run_async(&mut **client).await?;

        Ok(())
    }
}
