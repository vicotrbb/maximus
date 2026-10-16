use futures_util::{AsyncRead, AsyncWrite};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
pub struct Wire {
    pub input: Vec<u8>,
    pub position: usize,
    pub output: usize,
}
impl AsyncRead for Wire {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        output: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // Small reads exercise incremental boundaries, including chunk framing.
        let count = output.len().min(self.input.len() - self.position).min(17);
        output[..count].copy_from_slice(&self.input[self.position..self.position + count]);
        self.position += count;
        Poll::Ready(Ok(count))
    }
}
impl AsyncWrite for Wire {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.output += bytes.len();
        assert!(self.output <= 256 * 1024);
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
