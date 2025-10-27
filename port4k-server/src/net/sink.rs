pub mod telnet;
pub mod websocket;

use crate::net::output::OutFrame;
use async_trait::async_trait;

#[async_trait]
pub trait ClientSink: Send {
    async fn send_frame(&mut self, frame: OutFrame, seq: u64) -> anyhow::Result<()>;
}
