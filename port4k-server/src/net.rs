use std::sync::Arc;
use tokio::sync::mpsc;
use crate::lua::LuaJob;
use crate::Registry;

pub mod http;
pub mod telnet;

#[derive(Clone)]
pub struct AppState {
    /// Global registry with all services and repositories
    registry: Arc<Registry>,
    /// Channel to send jobs to the Lua thread
    lua_tx: mpsc::Sender<LuaJob>,
    /// Banner to show on connection
    banner: &'static str,
    /// Entry text to show on connection
    entry: &'static str,
}