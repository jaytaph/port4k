mod banner;
mod prelogin;
mod http;
mod db;
mod config;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;


use crate::banner::{BANNER, ENTRY};
use crate::prelogin::{handle_connection, Registry};


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cfg = config::Config::from_env()?;
    dbg!(&cfg);

    let db = db::Db::new(&cfg.database_url)?;
    db.init().await?;

    let tcp_addr: SocketAddr = cfg.tcp_addr.parse()?;
    let http_addr: SocketAddr = cfg.http_addr.parse()?;

    let listener = TcpListener::bind(tcp_addr).await?;
    tracing::info!(%tcp_addr, "Port4k server listening");

    let registry = Arc::new(Registry::new(db));

    let http_registry = registry.clone();
    tokio::spawn(async move {
        if let Err(e) = http::serve(SocketAddr::from(http_addr), http_registry, BANNER, ENTRY).await {
            eprintln!("HTTP server error: {e}");
        }
    });

    loop {
        let (stream, peer) = listener.accept().await?;
        let registry = registry.clone();
        tracing::info!(%peer, "client connected");
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, registry, BANNER, ENTRY).await {
                tracing::error!(%peer, error=%e, "connection error");
            }
            tracing::info!(%peer, "client disconnected");
        });
    }
}


fn init_tracing() {
    use tracing_subscriber::{prelude::*, EnvFilter};

    let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info,port4k_server=debug"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();
}