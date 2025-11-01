use axum::{
    Router,
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use futures::StreamExt;
use parking_lot::RwLock;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::banner::{BANNER, ENTRY};
use crate::commands::CmdCtx;
use crate::error::{AppResult, InfraError};
use crate::lua::LuaJob;
use crate::net::output::init_session_for_websocket;
use crate::state::session::Protocol;
use crate::{Registry, Session, process_command};
use tokio::sync::mpsc;

#[derive(Clone)]
struct HttpAppCtx {
    registry: Arc<Registry>,
    lua_tx: mpsc::Sender<LuaJob>,
}

/// Run the HTTP server with WebSocket endpoint
pub async fn serve(addr: std::net::SocketAddr, registry: Arc<Registry>, lua_tx: mpsc::Sender<LuaJob>) -> AppResult<()> {
    let app = Router::new()
        .route("/ws", get(ws_upgrade))
        .with_state(HttpAppCtx { registry, lua_tx })
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(InfraError::from)?;
    axum::serve(listener, app).await.map_err(InfraError::from)?;
    Ok(())
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<HttpAppCtx>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_handler(socket, state.registry.clone(), state.lua_tx.clone()))
}

async fn ws_handler(socket: WebSocket, registry: Arc<Registry>, lua_tx: mpsc::Sender<LuaJob>) {
    let (ws_write, mut ws_read) = socket.split();

    let sess = Arc::new(RwLock::new(Session::new(Protocol::WebSocket)));

    let io_bundle = init_session_for_websocket(ws_write, sess.clone()).await;

    io_bundle.output.system(BANNER).await;
    io_bundle.output.system(ENTRY).await;
    io_bundle.output.set_prompt("> ".to_string()).await;

    let ctx = Arc::new(CmdCtx {
        registry: registry.clone(),
        lua_tx: lua_tx.clone(),
        sess: sess.clone(),
        output: io_bundle.output.clone(),
    });

    while let Some(Ok(msg)) = ws_read.next().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Binary(b) => String::from_utf8_lossy(&b).to_string().into(),
            Message::Ping(_) => {
                // Axum already handles Pong responses automatically
                continue;
            }
            Message::Pong(_) => continue,
            Message::Close(_) => break,
        };

        let cmd = text.trim();
        if !cmd.is_empty() {
            _ = process_command(cmd, ctx.clone()).await;
        }
    }

    if let Ok(account) = ctx.account() {
        registry.set_online(&account, false).await;
    }
}

// fn ensure_nl(mut s: String) -> String {
//     if !s.ends_with("\n") {
//         s.push('\n');
//     }
//
//     s
// }
