use crate::Registry;
use crate::lua::LuaJob;
use crate::net::output::OutputHandle;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod http;
pub mod output;
pub mod sink;
pub mod telnet;

#[derive(Clone)]
struct AppCtx {
    output: OutputHandle,
    registry: Arc<Registry>,
    lua_tx: mpsc::Sender<LuaJob>,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Hidden(char),  // masked with for instance '*'
}