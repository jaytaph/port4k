use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub tcp_addr: String,       // e.g. "0.0.0.0:4000"
    pub websocket_addr: String, // e.g. "0.0.0.0:4001"
    pub database_url: String,   // e.g. "postgres://user:pass@localhost:5432/port4k"
    pub import_dir: String,
}

impl Config {
    #[allow(unused)]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&data)?;
        Ok(cfg)
    }

    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::from_filename(".env");
        let cfg = Self {
            tcp_addr: std::env::var("TCP_ADDR").unwrap_or_else(|_| "0.0.0.0:4000".to_string()),
            websocket_addr: std::env::var("WS_ADDR").unwrap_or_else(|_| "0.0.0.0:4001".to_string()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://user:pass@localhost:5432/port4k".to_string()),
            import_dir: std::env::var("IMPORT_DIR").unwrap_or_else(|_| "import".to_string()),
        };

        Ok(cfg)
    }
}
