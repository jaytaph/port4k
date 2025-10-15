mod connection;
mod crlf_wrapper;
mod slow_writer;

use crate::error::{AppResult, InfraError};
use crate::lua::LuaJob;
use crate::net::AppCtx;
use crate::net::telnet::connection::handle_connection;
use crate::state::session::Protocol;
use crate::{Registry, Session};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Run the telnet server
pub async fn serve(addr: std::net::SocketAddr, registry: Arc<Registry>, lua_tx: mpsc::Sender<LuaJob>) -> AppResult<()> {
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(InfraError::from)?;

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                tracing::info!(%peer, "client connected");

                let ctx = Arc::new(AppCtx {
                    registry: registry.clone(),
                    lua_tx: lua_tx.clone(),
                });

                let sess = Arc::new(RwLock::new(Session::new(Protocol::Telnet)));

                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, ctx.clone(), sess.clone()).await {
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
