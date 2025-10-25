use crate::Registry;
use crate::lua::LuaJob;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::net::output::OutputHandle;

pub mod http;
pub mod telnet;
pub mod output;
pub mod sink;

#[derive(Clone)]
struct AppCtx {
    output: OutputHandle,
    registry: Arc<Registry>,
    lua_tx: mpsc::Sender<LuaJob>,
}
