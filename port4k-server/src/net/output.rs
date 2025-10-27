use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use parking_lot::RwLock;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use crate::net::sink::ClientSink;
use crate::net::sink::telnet::TelnetSink;
use crate::net::sink::websocket::WebSocketSink;
use crate::renderer::render_template;
use crate::renderer::vars::generate_render_vars;
use crate::Session;

const MAX_TERMINAL_WIDTH: usize = 80;

#[derive(Debug, Clone)]
pub enum OutFrame {
    /// Regular "in-game" text line
    Line(String),
    /// System prompt from the game engine, not world related
    System(String),
    /// Room view content
    RoomView { content: String  },
    /// Display prompt line
    Prompt(String),
    /// Clear screen
    ClearScreen,
    /// Raw bytes for telnet IAC sequences
    Raw(Vec<u8>)
}

#[derive(Clone)]
pub struct OutputHandle {
    /// Sender for output events
    tx: mpsc::Sender<OutEvent>,
    /// Next sequence number for output frames
    next_seq: Arc<AtomicU64>,
    /// Session pointer
    sess: Arc<RwLock<Session>>,
}

impl OutputHandle {
    pub fn new(tx: mpsc::Sender<OutEvent>, session: Arc<RwLock<Session>>) -> Self {
        Self {
            tx,
            next_seq: Arc::new(AtomicU64::new(1)),
            sess: session.clone(),
        }
    }

    #[inline]
    pub fn next_seq(&self) -> u64 {
        self.next_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn line(&self, s: impl Into<String>) {
        let vars = generate_render_vars(self.sess.clone());
        let rendered = render_template(&s.into(), &vars, MAX_TERMINAL_WIDTH);
        let _ = self.tx.send(OutEvent::Frame(OutFrame::Line(rendered), self.next_seq())).await;
    }

    pub async fn system(&self, s: impl Into<String>) {
        let vars = generate_render_vars(self.sess.clone());
        let rendered = render_template(&s.into(), &vars, MAX_TERMINAL_WIDTH);
        let _ = self.tx.send(OutEvent::Frame(OutFrame::System(rendered), self.next_seq())).await;
    }

    pub async fn room_view(&self, content: impl Into<String>) {
        let vars = generate_render_vars(self.sess.clone());
        let rendered = render_template(&content.into(), &vars, MAX_TERMINAL_WIDTH);
        let _ = self.tx.send(OutEvent::Frame(OutFrame::RoomView { content: rendered }, self.next_seq())).await;
    }

    pub async fn prompt(&self, s: impl Into<String>) {
        let vars = generate_render_vars(self.sess.clone());
        let rendered = render_template(&s.into(), &vars, MAX_TERMINAL_WIDTH);
        let _ = self.tx.send(OutEvent::Frame(OutFrame::Prompt(rendered), self.next_seq())).await;
    }

    pub async fn raw(&self, bytes: Vec<u8>) {
        let _ = self.tx.send(OutEvent::Raw(bytes, self.next_seq())).await;
    }
}

pub enum OutEvent {
    /// A complete output frame with sequence number
    Frame(OutFrame, u64),
    /// Raw data for telnet IAC sequences with sequence number
    Raw(Vec<u8>, u64),
}

pub struct SessionOut {
    rx: mpsc::Receiver<OutEvent>,
}

impl SessionOut {
    pub fn new(rx: mpsc::Receiver<OutEvent>) -> Self {
        Self { rx }
    }

    pub async fn run<C>(mut self, mut client: C) -> anyhow::Result<()>
    where
        C: ClientSink
    {
        while let Some(event) = self.rx.recv().await {
            match event {
                OutEvent::Frame(frame, seq_nr) => client.send_frame(frame, seq_nr).await?,
                OutEvent::Raw(bytes, seq_nr) => {
                    // For telnet IAC sequences, we wrap them in an OutFrame::Raw
                    client.send_frame(OutFrame::Raw(bytes), seq_nr).await?
                }
            }
        }

        Ok(())
    }
}



pub struct SessionIoBundle {
    pub output: OutputHandle,
}

pub async fn init_session_for_telnet<W>(
    telnet_writer: W,
    sess: Arc<RwLock<Session>>,
) -> SessionIoBundle
where
    W: AsyncWrite + Unpin + Send + 'static
{
    let (tx, rx) = mpsc::channel::<OutEvent>(64);
    let output_handle = OutputHandle::new(tx, sess.clone());
    let session_out = SessionOut::new(rx);
    let sink = TelnetSink::new(telnet_writer);

    tokio::spawn(async move {
        if let Err(e) = session_out.run(sink).await {
            eprintln!("Session output error: {:?}", e);
        }
    });

    SessionIoBundle {
        output: output_handle,
    }
}

pub async fn init_session_for_websocket(
    websocket_writer: SplitSink<WebSocket, Message>,
    sess: Arc<RwLock<Session>>,
) -> SessionIoBundle {
    let (tx, rx) = mpsc::channel::<OutEvent>(64);
    let output_handle = OutputHandle::new(tx, sess);
    let session_out = SessionOut::new(rx);
    let sink = WebSocketSink::new(websocket_writer);

    tokio::spawn(async move {
        if let Err(e) = session_out.run(sink).await {
            eprintln!("Session output error: {:?}", e);
        }
    });

    SessionIoBundle {
        output: output_handle,
    }
}
