use deadpool_postgres::Pool;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Db {
    pub(crate) pool: Pool,
}

impl Db {
    #[allow(unused)]
    pub async fn get_client(&self) -> anyhow::Result<deadpool_postgres::Client> {
        Ok(self.pool.get().await?)
    }
}

// keep public API surface by re-exporting submodules
mod migrations;
mod pool;
pub mod types;

pub mod models;
pub mod accounts;
pub mod blueprint;
pub mod characters;
pub mod loot;
pub mod rooms;

pub mod repo;


fn json_string_vec(v: Option<Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items
            .into_iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}