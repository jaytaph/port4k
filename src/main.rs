use port4k::{
    Registry, config, db,
    lua::start_lua_worker,
    net::{http, telnet},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::runtime::Handle;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cfg = Arc::new(config::Config::from_env()?);

    let db = Arc::new(db::Db::new(&cfg.database_url)?);
    db.init().await?;

    let registry = Arc::new(Registry::new(db.clone(), cfg.clone()));

    let lua_tx = start_lua_worker(Handle::current(), registry.clone());

    // HTTP (WebSocket) server
    let ws_addr: SocketAddr = cfg.websocket_addr.parse()?;
    let ws_registry = registry.clone();
    let ws_lua_tx = lua_tx.clone();
    let ws_jh = tokio::spawn(async move {
        tracing::info!(%ws_addr, "Port4k server WS (http) listening");
        if let Err(e) = http::serve(ws_addr, ws_registry, ws_lua_tx).await {
            eprintln!("HTTP server error: {e}");
        }
    });

    // Telnet server
    let tcp_addr: SocketAddr = cfg.tcp_addr.parse()?;
    let tcp_registry = registry.clone();
    let tcp_lua_tx = lua_tx.clone();
    let tcp_jh = tokio::spawn(async move {
        tracing::info!(%tcp_addr, "Port4k server TCP (telnet) listening");
        if let Err(e) = telnet::serve(tcp_addr, tcp_registry, tcp_lua_tx).await {
            eprintln!("Telnet server error: {e}");
        }
    });

    // Wait for both (they only end on error)
    if let Err(e) = tokio::try_join!(ws_jh, tcp_jh) {
        tracing::error!(error=%e, "server task failed");
    }

    Ok(())
}

#[allow(unused)]
fn spawn_background_tasks(registry: Arc<Registry>) {
    let db_for_spawn = registry.db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(1000));
        loop {
            interval.tick().await;
            let _ = db_for_spawn.spawn_tick().await;
        }
    });
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, prelude::*};

    color_eyre::install().unwrap();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("debug".parse().unwrap()))
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_timer(tracing_subscriber::fmt::time::uptime()),
        )
        .with(tracing_error::ErrorLayer::default())
        .init();
}
