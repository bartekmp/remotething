//! A byte stream that counts what passes through it.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Totals of the bytes that crossed a stream, shared with whoever wants to read them.
#[derive(Clone, Default)]
pub(crate) struct Counters {
    pub(crate) received: Arc<AtomicU64>,
    pub(crate) sent: Arc<AtomicU64>,
}

impl Counters {
    pub(crate) fn received(&self) -> u64 {
        self.received.load(Ordering::Relaxed)
    }

    pub(crate) fn sent(&self) -> u64 {
        self.sent.load(Ordering::Relaxed)
    }
}

/// `inner`, with every byte counted.
pub(crate) struct Counted<S> {
    inner: S,
    counters: Counters,
}

impl<S> Counted<S> {
    pub(crate) fn new(inner: S, counters: Counters) -> Self {
        Self { inner, counters }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Counted<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let poll = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &poll {
            let read = buf.filled().len() - before;
            self.counters
                .received
                .fetch_add(read as u64, Ordering::Relaxed);
        }
        poll
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Counted<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let poll = Pin::new(&mut self.inner).poll_write(cx, data);
        if let Poll::Ready(Ok(written)) = &poll {
            self.counters
                .sent
                .fetch_add(*written as u64, Ordering::Relaxed);
        }
        poll
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

    #[tokio::test]
    async fn bytes_in_both_directions_are_counted() {
        let (near, mut far) = duplex(64);
        let counters = Counters::default();
        let mut counted = Counted::new(near, counters.clone());

        counted.write_all(b"hello").await.unwrap();
        far.write_all(b"abc").await.unwrap();
        let mut buffer = [0u8; 3];
        counted.read_exact(&mut buffer).await.unwrap();

        assert_eq!((counters.sent(), counters.received()), (5, 3));
    }
}
