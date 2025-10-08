use serde::Deserialize;
use std::path::{Path, PathBuf};
use crate::error::{ConfigErrorKind, InfraError};

/// Global configuration of the server
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub tcp_addr: String,       // e.g. "0.0.0.0:4000"
    pub websocket_addr: String, // e.g. "0.0.0.0:4001"
    pub database_url: String,   // e.g. "postgres://user:pass@localhost:5432/port4k"
    pub import_dir: String,
}

impl Config {
    #[allow(unused)]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, InfraError> {
        let path_buf = path.as_ref().to_path_buf();

        let data = std::fs::read_to_string(&path_buf).map_err(|e| InfraError::Config { path: path_buf.clone(), source: ConfigErrorKind::Read(e) })?;
        let cfg: Self = toml::from_str(&data).map_err(|e| InfraError::Config { path: path_buf.clone(), source: ConfigErrorKind::Parse(e) })?;

        Ok(cfg)
    }

    pub fn from_env() -> Result<Self, InfraError> {
        let _ = dotenvy::from_filename(".env");

        #[allow(unused)]
        fn req(key: &'static str) -> Result<String, InfraError> {
            std::env::var(key).map_err(|_| InfraError::Config {
                path: PathBuf::from(".env"),
                source: ConfigErrorKind::MissingEnv(key.to_string())
            })
        }
        fn opt(key: &'static str, default: &'static str) -> String {
            std::env::var(key).unwrap_or_else(|_| default.to_string())
        }

        let cfg = Self {
            tcp_addr: opt("TCP_ADDR", "0.0.0.0:4000"),
            websocket_addr: opt("WS_ADDR", "0.0.0.0:4001"),
            database_url: opt("DATABASE_URL", "postgres://user:pass@localhost:5432/port4k"),
            import_dir: opt("IMPORT_DIR", "import"),
            // important_token: req("IMPORTANT_TOKEN")?,
        };

        Ok(cfg)
    }
}
