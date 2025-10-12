use std::sync::Arc;
use tokio::sync::mpsc;
use crate::lua::LuaJob;
use crate::Registry;

pub mod http;
pub mod telnet;

#[derive(Clone)]
struct AppCtx {
    registry: Arc<Registry>,
    lua_tx: mpsc::Sender<LuaJob>,
}