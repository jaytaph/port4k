mod banner;
mod commands;
mod config;
mod db;
mod hardening;
mod import;
mod lua;
mod net;
mod rendering;
mod state;
mod util;
mod input;
mod ansi;
mod services;
mod error;
mod models;

pub use commands::process_command;
pub use state::{
    registry::Registry,
    session::{ConnState, Session},
};

use crate::lua::start_lua_worker;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::runtime::Handle;
use crate::net::telnet;
use crate::net::http;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cfg = Arc::new(config::Config::from_env()?);

    // Setup database and run migrations if needed
    let db = Arc::new(db::Db::new(&cfg.database_url)?);
    db.init().await?;

    let registry = Arc::new(Registry::new(db.clone(), cfg.clone()));

    // Start background tasks (spawning loot etc.)
    // spawn_background_tasks(registry.clone());

    // Start Lua worker thread
    let lua_tx = start_lua_worker(Handle::current(), registry.clone());


    // Start HTTP server for WebSocket connections
    let ws_addr: SocketAddr = cfg.websocket_addr.parse()?;
    let ws_registry = registry.clone();
    let ws_lua_tx = lua_tx.clone();
    let ws_jh = tokio::spawn(async move {
        tracing::info!(%ws_addr, "Port4k server WS (http) listening");

        if let Err(e) = http::serve(
            SocketAddr::from(ws_addr),
            ws_registry,
            ws_lua_tx,
        ).await {
            eprintln!("HTTP server error: {e}");
        }
    });

    // Start TCP server for Telnet connections
    let tcp_addr: SocketAddr = cfg.tcp_addr.parse()?;
    let tcp_registry = registry.clone();
    let tcp_lua_tx = lua_tx.clone();
    let tcp_jh = tokio::spawn(async move {
        tracing::info!(%tcp_addr, "Port4k server TCP (telnet) listening");

        if let Err(e) = telnet::serve(
            SocketAddr::from(tcp_addr),
            tcp_registry.clone(),
            tcp_lua_tx,
        ).await {
            eprintln!("Telnet server error: {e}");
        }
    });

    // Wait for both servers to finish (they won't, unless there's an error)
    match tokio::try_join!(ws_jh, tcp_jh) {
        Ok(_) => {}
        Err(e) => {
            tracing::error!(error=%e, "server task failed successfully");
        }
    }

    Ok(())
}

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
        .with(EnvFilter::from_default_env().add_directive("port4k=debug".parse().unwrap()))
        .with(tracing_subscriber::fmt::layer().with_target(false).with_timer(tracing_subscriber::fmt::time::uptime()))
        .with(tracing_error::ErrorLayer::default())
        .init();

    // let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
    // let filter = EnvFilter::try_from_default_env()
    //     .or_else(|_| EnvFilter::try_new("info,port4k_server=debug"))
    //     .unwrap();
    // tracing_subscriber::registry().with(filter).with(fmt_layer).init();
}
