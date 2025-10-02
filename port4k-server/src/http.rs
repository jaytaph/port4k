use axum::{
    Router,
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use std::sync::{Arc, RwLock};
use tower_http::cors::{Any, CorsLayer};

use crate::lua::LuaJob;
use crate::{Registry, Session, process_command};
use tokio::sync::mpsc;
use crate::commands::CommandResult;

/// Serve the HTTP server with WebSocket endpoint
pub async fn serve(
    addr: std::net::SocketAddr,
    registry: Arc<Registry>,
    banner: &'static str,
    entry: &'static str,
    lua_tx: mpsc::Sender<LuaJob>,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/ws", get(ws_upgrade))
        .with_state(AppState {
            registry,
            banner,
            entry,
            lua_tx,
        })
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

    let sess = Arc::new(RwLock::new(Session::default()));

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
        match process_command(cmd, state.registry.clone(), sess.clone(), state.lua_tx.clone()).await {
            Ok(CommandResult::Success(msg)) => {
                let resp = if msg.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", msg)
                };
                let _ = socket
                    .send(Message::Text(format!("{}> ", ensure_nl(resp))))
                    .await;
                continue;
            }
            Ok(CommandResult::Failure(msg)) => {
                let _ = socket
                    .send(Message::Text(format!("error: {msg}\n{}> ", state.entry)))
                    .await;
                continue;
            }
            Err(e) => {
                let _ = socket
                    .send(Message::Text(format!("error: {e}\n{}> ", state.entry)))
                    .await;
                continue;
            }
        }

        // if matches!(cmd.to_ascii_lowercase().as_str(), "quit" | "exit") {
        //     let _ = socket.close().await;
        //     break;
        // }
    }

    let account = {
        let s = sess.read().unwrap();
        s.account.clone()
    };
    if let Some(a) = account {
        state.registry.set_online(&a, false).await;
    }
}

fn ensure_nl(mut s: String) -> String {
    if !s.ends_with("\n") {
        s.push('\n');
    }
    s
}
