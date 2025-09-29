mod banner;
mod http;
mod db;
mod config;
mod util;
mod lua;
mod state;
mod net;
mod commands;
mod scripting;

pub use net::connection::handle_connection;
pub use state::{session::{Session, ConnState, WorldMode, Editor}, registry::Registry};
pub use commands::process_command;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use crate::banner::{BANNER, ENTRY};
use crate::lua::start_lua_worker;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cfg = config::Config::from_env()?;

    // Setup database and run migrations if needed
    let db = db::Db::new(&cfg.database_url)?;
    db.init().await?;

    // Start background tasks (spawning loot etc.)
    spawn_background_tasks(db.clone());

    let tcp_addr: SocketAddr = cfg.tcp_addr.parse()?;
    let listener = TcpListener::bind(tcp_addr).await?;
    tracing::info!(%tcp_addr, "Port4k server listening");

    let registry = Arc::new(Registry::new(db));

    // Start Lua worker thread
    let lua_tx = start_lua_worker(Handle::current());

    // Start HTTP server for WebSocket connections
    let websocket_addr: SocketAddr = cfg.websocket_addr.parse()?;
    let http_registry = registry.clone();
    let lua_tx_for_http = lua_tx.clone();
    let http_jh = tokio::spawn(async move {
        if let Err(e) = http::serve(SocketAddr::from(websocket_addr), http_registry, BANNER, ENTRY, lua_tx_for_http).await {
            eprintln!("HTTP server error: {e}");
        }
    });

    let telnet_registry = Arc::clone(&registry);
    let telnet_jh = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    tracing::info!(%peer, "client connected");
                    let registry = telnet_registry.clone();
                    let lua_tx_clone = lua_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, registry, BANNER, ENTRY, lua_tx_clone.clone()).await {
                            tracing::error!(%peer, error=%e, "connection error");
                        }
                        tracing::info!(%peer, "client disconnected");
                    });
                }
                Err(e) => {
                    tracing::error!(error=%e, "failed to accept connection");
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }
    });

    // Wait for both servers to finish (they won't, unless there's an error)
    match tokio::try_join!(http_jh, telnet_jh) {
        Ok(_) => {},
        Err(e) => {
            tracing::error!(error=%e, "server task failed successfully");
        }
    }

    Ok(())
}

fn spawn_background_tasks(db: db::Db) {
    let db_for_spawn = db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(1000));
        loop {
            interval.tick().await;
            let _ = db_for_spawn.spawn_tick().await;
        }
    });
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