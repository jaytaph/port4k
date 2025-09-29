use std::sync::Arc;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::prelogin::{process_command, Registry, Session};
use tokio::sync::Mutex;

pub async fn serve(
    addr: std::net::SocketAddr,
    registry: Arc<Registry>,
    banner: &'static str,
    entry: &'static str,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_upgrade))
        .with_state(AppState { registry, banner, entry })
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
struct AppState {
    registry: Arc<Registry>,
    banner: &'static str,
    entry: &'static str,
}

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
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
        let resp = process_command(cmd, &state.registry, &sess)
            .await
            .unwrap_or_else(|e| format!("erreur: {e}\\n"));

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

const INDEX_HTML: &str = r#"
<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <title>Port4k</title>
  <style>
    body{background:#0d0f14;color:#c7d0dc;font-family:monospace}
    #log{white-space:pre-wrap;padding:16px}
    #bar{position:fixed;bottom:0;left:0;right:0;padding:8px;background:#111625}
    input{width:100%;padding:10px;background:#0f1320;color:#e6eef8;border:1px solid #1d2236}
  </style>
</head>
<body>
  <div id="log"></div>
  <div id="bar"><input id="in" placeholder="Type commands… (help)"></div>
  <script>
    const log=document.getElementById('log');
    const input=document.getElementById('in');
    const ws=new WebSocket((location.protocol==='https:'?'wss://':'ws://')+location.host+'/ws');
    const append=(t)=>{log.textContent+=t;window.scrollTo(0,document.body.scrollHeight);};
    ws.addEventListener('message',(ev)=>append(ev.data));
    input.addEventListener('keydown',(e)=>{if(e.key==='Enter'){ws.send(input.value);input.value='';}});
  </script>
</body>
</html>
"#;
