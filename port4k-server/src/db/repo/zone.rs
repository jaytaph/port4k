use crate::db::DbResult;
use crate::models::zone::Zone;

#[async_trait::async_trait]
pub trait ZoneRepo: Send + Sync {
    async fn get_by_key(&self, zone_key: &str) -> DbResult<Option<Zone>>;
}
