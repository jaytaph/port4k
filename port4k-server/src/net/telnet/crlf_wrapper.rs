use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::io::{self, AsyncWrite};

/// Normalizes bare '\n' to "\r\n" on all writes.
///
/// Note: we only report the input `buf.len()` as written after the
/// expanded data has fully flushed to the inner writer.
pub struct CrlfWriter<W> {
    inner: W,
    // Buffered, expanded output awaiting flush
    out_buf: Vec<u8>,
    out_pos: usize,
}

impl<W: AsyncWrite + Unpin> CrlfWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner, out_buf: Vec::new(), out_pos: 0 }
    }
    // pub fn into_inner(self) -> W { self.inner }

    #[inline]
    fn expand_crlf(dst: &mut Vec<u8>, src: &[u8]) {
        dst.reserve(src.len()); // heuristic; '\n' may add extra bytes
        for &b in src {
            if b == b'\n' {
                dst.push(b'\r');
                dst.push(b'\n');
            } else {
                dst.push(b);
            }
        }
    }

    /// Try to flush `out_buf` into inner.
    fn poll_flush_outbuf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = unsafe { self.get_unchecked_mut() };

        while this.out_pos < this.out_buf.len() {
            let n = ready!(Pin::new(&mut this.inner).poll_write(cx, &this.out_buf[this.out_pos..]))?;
            if n == 0 {
                // EOF or would-block; treat as WouldBlock to avoid busy loop
                return Poll::Pending;
            }
            this.out_pos += n;
        }

        // fully flushed; reset buffer
        this.out_buf.clear();
        this.out_pos = 0;

        Poll::Ready(Ok(()))
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for CrlfWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // If we still have expanded bytes pending, flush them first.
        if !self.out_buf.is_empty() {
            match self.as_mut().poll_flush_outbuf(cx) {
                Poll::Ready(Ok(())) => { /* flushed, continue */ }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }

        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        // Expand the incoming chunk into out_buf and try to flush it.
        Self::expand_crlf(&mut self.out_buf, buf);

        match self.as_mut().poll_flush_outbuf(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            // Not fully flushed yet; report Pending and consume nothing from caller.
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Ensure our own buffer is empty, then flush inner.
        match self.as_mut().poll_flush_outbuf(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut self.inner).poll_flush(cx),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Flush our buffer first, then shutdown inner.
        match self.as_mut().poll_flush_outbuf(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut self.inner).poll_shutdown(cx),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}
