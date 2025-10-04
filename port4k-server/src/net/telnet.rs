mod connection;
mod crlf_wrapper;

use std::sync::{Arc, RwLock};
use crate::lua::LuaJob;
use crate::{Registry, Session};
use tokio::sync::mpsc;
use crate::net::AppState;
use crate::net::telnet::connection::handle_connection;
use crate::state::session::Protocol;

/// Run the telnet server
pub async fn serve(
    addr: std::net::SocketAddr,
    registry: Arc<Registry>,
    banner: &'static str,
    entry: &'static str,
    lua_tx: mpsc::Sender<LuaJob>,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                tracing::info!(%peer, "client connected");

                let state = Arc::new(AppState {
                    registry: registry.clone(),
                    lua_tx: lua_tx.clone(),
                    banner,
                    entry,
                });

                let sess = Arc::new(RwLock::new(Session::new(Protocol::Telnet)));

                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, state.clone(), sess.clone()).await {
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
}



