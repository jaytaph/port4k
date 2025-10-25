pub mod telnet;
pub mod websocket;

use async_trait::async_trait;
use crate::net::output::OutFrame;

#[async_trait]
pub trait ClientSink: Send {
    async fn send_frame(&mut self, frame: OutFrame, seq: u64) -> anyhow::Result<()>;
}
