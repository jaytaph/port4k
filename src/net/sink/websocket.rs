use crate::net::output::OutFrame;
use crate::net::sink::ClientSink;
use async_trait::async_trait;
use futures::SinkExt;
use serde::Serialize;

pub struct WebSocketSink<S, M> {
    ws: S,
    _phantom: std::marker::PhantomData<M>,
}

impl<S, M> WebSocketSink<S, M> {
    pub fn new(ws: S) -> Self {
        Self {
            ws,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WsFrame<'a> {
    Line { text: &'a str },
    System { text: &'a str },
    RoomView { content: &'a str },
    Prompt { text: &'a str },
    ClearScreen,
}

#[derive(Serialize)]
struct WsEnvelope<T> {
    seq: u64,
    frame: T,
}

#[async_trait]
impl<S, M> ClientSink for WebSocketSink<S, M>
where
    S: SinkExt<M> + Unpin + Send,
    S::Error: std::error::Error + Send + Sync + 'static,
    M: From<String> + Send,
{
    async fn send_frame(&mut self, frame: OutFrame, seq: u64) -> anyhow::Result<()> {
        let payload = match &frame {
            OutFrame::Line(s) => WsFrame::Line { text: s },
            OutFrame::System(s) => WsFrame::System { text: s },
            OutFrame::RoomView { content } => WsFrame::RoomView { content },
            OutFrame::Prompt(s) => WsFrame::Prompt { text: s },
            OutFrame::InputMode(_) => {
                return Err(anyhow::Error::msg(
                    "InputMode frame not supported over WebSocket sink",
                ));
            }
            OutFrame::ClearScreen => WsFrame::ClearScreen,
            OutFrame::Raw(_) => {
                return Err(anyhow::Error::msg("Raw frame not supported over WebSocket sink"));
            }
            OutFrame::RepaintLine(line) => WsFrame::Line { text: line },
        };

        let env = WsEnvelope { seq, frame: payload };
        let json = serde_json::to_string(&env)?;

        self.ws
            .send(json.into())
            .await
            .map_err(|e| anyhow::Error::msg(format!("websocket send failed: {e}")))?;

        Ok(())
    }
}
