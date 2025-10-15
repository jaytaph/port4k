use crate::Registry;
use crate::lua::LuaJob;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod http;
pub mod telnet;

#[derive(Clone)]
struct AppCtx {
    registry: Arc<Registry>,
    lua_tx: mpsc::Sender<LuaJob>,
}
