use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::AsyncWrite;
use tokio::time::{Sleep, sleep};

#[allow(unused)]
pub enum Pace {
    PerChar {
        delay: Duration,
    },
    PerWord {
        delay: Duration,
    },
}

pub struct SlowWriter<W> {
    inner: W,
    pace: Pace,
    sleep: Option<Pin<Box<Sleep>>>,
    pacing_enabled: bool, // NEW
}

impl<W> SlowWriter<W> {
    #[allow(unused)]
    pub fn new(inner: W, pace: Pace) -> Self {
        Self {
            inner,
            pace,
            sleep: None,
            pacing_enabled: true,
        }
    }

    /// Enable/disable pacing globally.
    #[allow(unused)]
    pub fn set_pacing(&mut self, enabled: bool) {
        self.pacing_enabled = enabled;
        if !enabled {
            // Cancel any in-flight sleep so we don't block a fast write.
            self.sleep = None;
        }
    }

    // /// Temporarily disable pacing for the duration of the guard.
    // pub fn pause_pacing(&mut self) -> PacingGuard<'_, W> {
    //     let was = self.pacing_enabled;
    //     self.set_pacing(false);
    //     PacingGuard { w: self, prev: was }
    // }
}

// pub struct PacingGuard<'a, W> {
//     w: &'a mut SlowWriter<W>,
//     prev: bool,
// }
// impl<'a, W> Drop for PacingGuard<'a, W> {
//     fn drop(&mut self) {
//         self.w.set_pacing(self.prev);
//     }
// }

impl<W: AsyncWrite + Unpin> AsyncWrite for SlowWriter<W> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        // FAST PATH: pacing disabled ⇒ pass through directly.
        if !self.pacing_enabled {
            return Pin::new(&mut self.inner).poll_write(cx, buf);
        }

        // Handle active delay
        if let Some(sleep) = self.sleep.as_mut() {
            match sleep.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(_) => {
                    self.sleep = None;
                }
            }
        }

        let next_len = next_chunk_len(buf, &self.pace);
        let n = futures::ready!(Pin::new(&mut self.inner).poll_write(cx, &buf[..next_len]))?;

        if n > 0 && n < buf.len() {
            let delay = match self.pace {
                Pace::PerChar { delay } | Pace::PerWord { delay } => delay,
            };
            self.sleep = Some(Box::pin(sleep(delay)));
        }

        Poll::Ready(Ok(n))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

fn next_chunk_len(buf: &[u8], pace: &Pace) -> usize {
    if let Some(len) = ansi_prefix_len(buf) {
        return len.max(1);
    }

    match *pace {
        Pace::PerChar { .. } => 1,
        Pace::PerWord { .. } => {
            if buf[0] == b'\n' || buf[0] == b'\r' {
                return 1;
            }
            if is_ws(buf[0]) {
                return 1;
            }
            if is_alnum(buf[0]) {
                let mut i = 0usize;
                while i < buf.len() && !is_ws(buf[i]) {
                    if i + 1 < buf.len() && buf[i] == 0x1B && buf[i + 1] == b'[' {
                        break;
                    }
                    i += 1;
                }
                return i.max(1);
            }
            // Non-alnum: emit until newline (ASCII-art line)
            let mut i = 0usize;
            while i < buf.len() {
                if i + 1 < buf.len() && buf[i] == 0x1B && buf[i + 1] == b'[' {
                    break;
                }
                if buf[i] == b'\n' || buf[i] == b'\r' {
                    break;
                }
                i += 1;
            }
            i.max(1)
        }
    }
}
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}
fn is_alnum(b: u8) -> bool {
    b.is_ascii_uppercase() || b.is_ascii_lowercase() || b.is_ascii_digit()
}
fn ansi_prefix_len(buf: &[u8]) -> Option<usize> {
    if buf.len() >= 2 && buf[0] == 0x1B && buf[1] == b'[' {
        let mut i = 2;
        while i < buf.len() {
            let b = buf[i];
            if (0x40..=0x7E).contains(&b) {
                return Some(i + 1);
            }
            i += 1;
        }
        return Some(buf.len());
    }
    None
}
