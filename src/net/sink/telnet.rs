use crate::net::InputMode;
use crate::net::output::OutFrame;
use crate::net::sink::ClientSink;
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;

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
            OutFrame::InputMode(InputMode::Normal) => {
                // Client may echo again
                self.writer.write_all(&[255, 252, 1]).await?; // IAC wont echo
            }
            OutFrame::InputMode(InputMode::Hidden(_mask)) => {
                // No echo
                self.writer.write_all(&[255, 251, 1]).await?; // IAC will echo
            }
            OutFrame::Line(s) => {
                for line in s.lines() {
                    // normal line + newline
                    self.writer.write_all(line.as_bytes()).await?;
                    self.writer.write_all(b"\r\n").await?;
                }
                self.writer.write_all(b"\r\n").await?;
            }
            OutFrame::System(s) => {
                let lines: Vec<&str> = s.lines().collect();
                let mut last_color = String::new(); // ansi color code tracking
                for line in &lines {
                    self.writer.write_all(b"\x1b[93m[SRV]\x1b[0m ").await?; // yellow [SRV] marker

                    if !last_color.is_empty() {
                        self.writer.write_all(last_color.as_bytes()).await?;
                    }

                    self.writer.write_all(line.as_bytes()).await?;
                    self.writer.write_all(b"\x1b[0m\r\n").await?;

                    if let Some(code) = extract_last_color_code(line) {
                        last_color = code
                    }
                }
                self.writer.write_all(b"\r\n").await?;
            }
            OutFrame::RoomView { content } => {
                // Typically you'd clear top section or draw a frame.
                // For first pass we just dump it:
                self.writer.write_all(content.as_bytes()).await?;
                self.writer.write_all(b"\r\n").await?;
            }
            OutFrame::Prompt(p) => {
                // Write prompt, but DON'T newline.
                // Also, maybe re-show input buffer later.
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
            OutFrame::RepaintLine(line) => {
                // Carriage return, clear line, write line
                self.writer.write_all(b"\r\x1b[0K").await?;
                self.writer.write_all(line.as_bytes()).await?;
            }
        }

        Ok(())
    }
}

/// Extract the last SGR (Select Graphic Rendition) ANSI escape code from a line.
fn extract_last_color_code(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut last_sgr = None;
    let mut i = 0;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'\x1b' && bytes[i + 1] == b'[' {
            let start = i;
            i += 2;

            while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                i += 1;
            }

            if i < bytes.len() && bytes[i] == b'm' {
                // Found \x1b[...m sequence
                let sequence = &line[start..=i];

                let params = &line[start + 2..i];
                if params.is_empty() || params == "0" {
                    last_sgr = Some(String::new()); // Reset clears color
                } else {
                    last_sgr = Some(sequence.to_string());
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }

    last_sgr
}
