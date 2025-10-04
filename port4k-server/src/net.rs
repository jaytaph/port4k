use std::sync::Arc;
use tokio::sync::mpsc;
use crate::lua::LuaJob;
use crate::Registry;

pub mod http;
pub mod telnet;

#[derive(Clone)]
pub struct AppState {
    /// Global registry with all services and repositories
    pub registry: Arc<Registry>,
    /// Channel to send jobs to the Lua thread
    pub lua_tx: mpsc::Sender<LuaJob>,
    /// Banner to show on connection
    pub banner: &'static str,
    /// Entry text to show on connection
    pub entry: &'static str,
}