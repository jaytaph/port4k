use axum::{
    Router,
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use parking_lot::RwLock;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::banner::{BANNER, ENTRY};
use crate::commands::CmdCtx;
use crate::error::{AppResult, InfraError};
use crate::lua::LuaJob;
use crate::net::AppCtx;
use crate::state::session::Protocol;
use crate::{Registry, Session, process_command};
use tokio::sync::mpsc;

/// Run the HTTP server with WebSocket endpoint
pub async fn serve(addr: std::net::SocketAddr, registry: Arc<Registry>, lua_tx: mpsc::Sender<LuaJob>) -> AppResult<()> {
    let app = Router::new()
        .route("/ws", get(ws_upgrade))
        .with_state(AppCtx { registry, lua_tx })
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(InfraError::from)?;
    axum::serve(listener, app).await.map_err(InfraError::from)?;
    Ok(())
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppCtx>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_handler(socket, state.registry.clone(), state.lua_tx.clone()))
}

async fn ws_handler(mut socket: WebSocket, registry: Arc<Registry>, lua_tx: mpsc::Sender<LuaJob>) {
    let _ = socket.send(Message::Text(format!("{}{}> ", BANNER, ENTRY).into())).await;

    let sess = Arc::new(RwLock::new(Session::new(Protocol::WebSocket)));

    let ctx = Arc::new(CmdCtx {
        registry: registry.clone(),
        lua_tx: lua_tx.clone(),
        sess: sess.clone(),
    });

    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Binary(b) => String::from_utf8_lossy(&b).to_string().into(),
            Message::Ping(p) => {
                let _ = socket.send(Message::Pong(p)).await;
                continue;
            }
            Message::Pong(_) => continue,
            Message::Close(_) => break,
        };

        let cmd = text.trim();
        match process_command(cmd, ctx.clone()).await {
            Ok(res) => {
                let resp = if res.failed() {
                    format!("error: {}\n", res.message())
                } else {
                    format!("{}\n", res.message())
                };
                let _ = socket.send(Message::Text(format!("{}> ", ensure_nl(resp)).into())).await;
            }
            Err(e) => {
                let _ = socket.send(Message::Text(format!("error: {}\n> ", e).into())).await;
            }
        }

        // if matches!(cmd.to_ascii_lowercase().as_str(), "quit" | "exit") {
        //     let _ = socket.close().await;
        //     break;
        // }
    }

    if let Ok(account) = ctx.account() {
        registry.set_online(&account, false).await;
    }
}

fn ensure_nl(mut s: String) -> String {
    if !s.ends_with("\n") {
        s.push('\n');
    }

    s
}
