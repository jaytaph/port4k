use crate::db::repo::zone::ZoneRepo;
use crate::db::{Db, DbResult};
use crate::models::zone::Zone;
use std::sync::Arc;

pub struct ZoneRepository {
    db: Arc<Db>,
}

impl ZoneRepository {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db: db.clone() }
    }
}

#[async_trait::async_trait]
impl ZoneRepo for ZoneRepository {
    async fn get_by_key(&self, zone_key: &str) -> DbResult<Option<Zone>> {
        let client = self.db.get_client().await?;

        let stmt = client
            .prepare_cached(
                r#"
            SELECT id, key, title, kind, created_at
            FROM zones
            WHERE key = $1
        "#,
            )
            .await?;

        let row_opt = client.query_opt(&stmt, &[&zone_key]).await?;
        row_opt.as_ref().map(Zone::try_from_row).transpose()
    }
}
