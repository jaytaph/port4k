use axum::{
    Router,
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use std::sync::Arc;
use parking_lot::RwLock;
use tower_http::cors::{Any, CorsLayer};

use crate::lua::LuaJob;
use crate::{Registry, Session, process_command};
use tokio::sync::mpsc;
use crate::commands::CmdCtx;
use crate::error::{AppResult, InfraError};
use crate::net::AppState;
use crate::state::session::Protocol;

/// Run the HTTP server with WebSocket endpoint
pub async fn serve(
    addr: std::net::SocketAddr,
    registry: Arc<Registry>,
    banner: &'static str,
    entry: &'static str,
    lua_tx: mpsc::Sender<LuaJob>,
) -> AppResult<()> {
    let app = Router::new()
        .route("/ws", get(ws_upgrade))
        .with_state(AppState {
            registry,
            banner,
            entry,
            lua_tx,
        })
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(InfraError::from)?;
    axum::serve(listener, app).await.map_err(InfraError::from)?;
    Ok(())
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_handler(socket, state))
}

async fn ws_handler(mut socket: WebSocket, state: AppState) {
    let _ = socket
        .send(Message::Text(format!("{}{}> ", state.banner, state.entry)))
        .await;

    let state = Arc::new(state);
    let sess = Arc::new(RwLock::new(Session::new(Protocol::WebSocket)));

    let ctx = CmdCtx {
        state: state.clone(),
        sess: sess.clone(),
    };

    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Binary(b) => String::from_utf8_lossy(&b).to_string(),
            Message::Ping(p) => {
                let _ = socket.send(Message::Pong(p)).await;
                continue;
            }
            Message::Pong(_) => continue,
            Message::Close(_) => break,
        };

        let cmd = text.trim();
        match process_command(cmd, state.clone(), sess.clone()).await {
            Ok(res) => {
                let resp = if res.is_error {
                    format!("error: {}\n", res.message)
                } else {
                    format!("{}\n", res.message)
                };
                let _ = socket
                    .send(Message::Text(format!("{}> ", ensure_nl(resp))))
                    .await;
            }
            Err(e) => {
                let _ = socket
                    .send(Message::Text(format!("error: {}\n> ", e)))
                    .await;
            }
        }

        // if matches!(cmd.to_ascii_lowercase().as_str(), "quit" | "exit") {
        //     let _ = socket.close().await;
        //     break;
        // }
    }

    if let Ok(account) = ctx.account() {
        state.registry.set_online(&account, false).await;
    }
}

fn ensure_nl(mut s: String) -> String {
    if !s.ends_with("\n") {
        s.push('\n');
    }

    s
}
