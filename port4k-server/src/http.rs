use std::sync::Arc;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use tokio::sync::{mpsc, Mutex};
use crate::lua::LuaJob;
use crate::{process_command, Registry, Session};

pub async fn serve(
    addr: std::net::SocketAddr,
    registry: Arc<Registry>,
    banner: &'static str,
    entry: &'static str,
    lua_tx: mpsc::Sender<LuaJob>,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/ws", get(ws_upgrade))
        .with_state(AppState { registry, banner, entry, lua_tx })
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
struct AppState {
    registry: Arc<Registry>,
    lua_tx: mpsc::Sender<LuaJob>,
    banner: &'static str,
    entry: &'static str,
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_handler(socket, state))
}

async fn ws_handler(mut socket: WebSocket, state: AppState) {
    let _ = socket
        .send(Message::Text(format!("{}{}> ", state.banner, state.entry)))
        .await;

    let sess = Arc::new(Mutex::new(Session::default()));

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
        let resp = process_command(cmd, &state.registry, &sess, state.lua_tx.clone())
            .await
            .unwrap_or_else(|e| format!("error: {e}\\n"));

        let _ = socket
            .send(Message::Text(format!("{}> ", ensure_nl(resp))))
            .await;

        if matches!(cmd.to_ascii_lowercase().as_str(), "quit" | "exit") {
            let _ = socket.close().await;
            break;
        }
    }

    if let Some(u) = sess.lock().await.name.clone() {
        state.registry.set_online(&u, false).await;
    }
}

fn ensure_nl(mut s: String) -> String {
    if !s.ends_with("\n") {
        s.push('\n');
    }
    s
}
