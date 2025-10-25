use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use crate::net::output::OutFrame;
use crate::net::sink::ClientSink;

pub struct TelnetSink<W> {
    writer: W,
}

impl<W> TelnetSink<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

#[async_trait]
impl<W> ClientSink for TelnetSink<W>
where
    W: AsyncWriteExt + Unpin + Send,
{
    async fn send_frame(&mut self, frame: OutFrame, _seq: u64) -> anyhow::Result<()> {
        match frame {
            OutFrame::Line(s) => {
                // normal line + newline
                self.writer.write_all(s.as_bytes()).await?;
                self.writer.write_all(b"\r\n").await?;
            }
            OutFrame::System(s) => {
                // maybe prefix with some ANSI color for "system"?
                self.writer.write_all(b"\x1b[33m").await?; // yellow
                self.writer.write_all(s.as_bytes()).await?;
                self.writer.write_all(b"\x1b[0m\r\n").await?;
            }
            OutFrame::RoomView { content } => {
                // Typically you'd clear top section or draw a frame.
                // For first pass we just dump it:
                self.writer.write_all(content.as_bytes()).await?;
                self.writer.write_all(b"\r\n").await?;
            }
            OutFrame::Prompt(p) => {
                // Write prompt, but DON'T newline.
                // Also maybe re-show input buffer later.
                self.writer.write_all(p.as_bytes()).await?;
            }
            OutFrame::ClearScreen => {
                // ANSI clear
                self.writer.write_all(b"\x1b[2J\x1b[H").await?;
            }
            OutFrame::Raw(bytes) => {
                // Directly write raw bytes (e.g. telnet IAC sequences)
                self.writer.write_all(&bytes).await?;
            }
        }

        Ok(())
    }
}