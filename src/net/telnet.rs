mod connection;
mod crlf_wrapper;
mod slow_writer;

use crate::banner::{BANNER, ENTRY};
use crate::error::{AppResult, InfraError};
use crate::lua::LuaJob;
use crate::net::AppCtx;
use crate::net::output::init_session_for_telnet;
use crate::net::telnet::connection::handle_connection;
use crate::net::telnet::crlf_wrapper::CrlfWriter;
use crate::state::session::Protocol;
use crate::util::telnet::TelnetMachine;
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

                // let ctx = Arc::new(AppCtx {
                //     registry: registry.clone(),
                //     lua_tx: lua_tx.clone(),
                // });

                // let sess = Arc::new(RwLock::new(Session::new(Protocol::Telnet)));

                let lua_tx = lua_tx.clone();
                let registry = registry.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_telnet_connection(stream, peer, registry.clone(), lua_tx.clone()).await {
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

async fn handle_telnet_connection(
    stream: tokio::net::TcpStream,
    _peer: std::net::SocketAddr,
    registry: Arc<Registry>,
    lua_tx: mpsc::Sender<LuaJob>,
) -> AppResult<()> {
    let (read_half, write_half) = stream.into_split();

    // Wrap write half with CRLF conversion and pacing
    let crlf_writer = CrlfWriter::new(write_half);
    // let mut paced_writer = SlowWriter::new(
    //     crlf_writer,
    //     Pace::PerWord {
    //         delay: Duration::from_millis(1),
    //     }
    // );
    let mut wrapper_writer = crlf_writer;

    // Telnet option negotiation: character-at-a-time + SGA + (server) echo + NAWS
    let mut telnet = TelnetMachine::new();
    telnet.start_negotiation(&mut wrapper_writer).await?;

    let sess = Arc::new(RwLock::new(Session::new(Protocol::Telnet)));

    let io_bundle = init_session_for_telnet(wrapper_writer, sess.clone()).await;

    io_bundle.output.system(BANNER).await;
    io_bundle.output.system(ENTRY).await;
    // io_bundle.output.prompt("> ".to_string()).await;

    let ctx = Arc::new(AppCtx {
        registry: registry.clone(),
        lua_tx: lua_tx.clone(),
        output: io_bundle.output.clone(),
    });
    handle_connection(read_half, ctx, &mut telnet, sess).await?;

    Ok(())
}
