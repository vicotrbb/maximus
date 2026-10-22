//! The TCP path submits owned buffers directly. TLS keeps its explicit futures adapter.
use compio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
#[cfg(feature = "websocket")]
use std::pin::Pin;
use std::{io, ops::Range};

pub(crate) trait ReadIo: 'static {
    /// Read into initialized storage without consuming its prefix.
    async fn read(&mut self, buffer: Vec<u8>, range: Range<usize>) -> (io::Result<usize>, Vec<u8>);
    #[cfg(feature = "websocket")]
    fn into_futures(self, capacity: usize) -> Pin<Box<dyn futures_util::AsyncRead>>;
}
pub(crate) trait WriteIo: 'static {
    async fn write(&mut self, buffer: Vec<u8>) -> (io::Result<()>, Vec<u8>);
    async fn flush(&mut self) -> io::Result<()>;
    async fn close(&mut self) -> io::Result<()>;
    #[cfg(feature = "websocket")]
    fn into_futures(self, capacity: usize) -> Pin<Box<dyn futures_util::AsyncWrite>>;
}
pub(crate) struct Native<T>(pub T);
impl<T: AsyncRead + Unpin + 'static> ReadIo for Native<T> {
    async fn read(&mut self, buffer: Vec<u8>, range: Range<usize>) -> (io::Result<usize>, Vec<u8>) {
        use compio::buf::{IntoInner, IoBuf};
        let compio::BufResult(result, buffer) = self.0.read(buffer.slice(range)).await;
        (result, buffer.into_inner())
    }
    #[cfg(feature = "websocket")]
    fn into_futures(self, capacity: usize) -> Pin<Box<dyn futures_util::AsyncRead>> {
        Box::pin(compio::io::compat::AsyncStream::with_limits(
            capacity,
            capacity,
            (self.0, compio::io::null()),
        ))
    }
}
impl<T: AsyncWrite + Unpin + 'static> WriteIo for Native<T> {
    async fn write(&mut self, buffer: Vec<u8>) -> (io::Result<()>, Vec<u8>) {
        let compio::BufResult(result, buffer) = self.0.write_all(buffer).await;
        (result, buffer)
    }
    async fn flush(&mut self) -> io::Result<()> {
        self.0.flush().await
    }
    async fn close(&mut self) -> io::Result<()> {
        self.0.shutdown().await
    }
    #[cfg(feature = "websocket")]
    fn into_futures(self, capacity: usize) -> Pin<Box<dyn futures_util::AsyncWrite>> {
        Box::pin(compio::io::compat::AsyncStream::with_limits(
            capacity,
            capacity,
            (compio::io::null(), self.0),
        ))
    }
}
pub(crate) struct Futures<T>(pub T);
impl<T: futures_util::AsyncRead + Unpin + 'static> ReadIo for Futures<T> {
    async fn read(
        &mut self,
        mut buffer: Vec<u8>,
        range: Range<usize>,
    ) -> (io::Result<usize>, Vec<u8>) {
        let result = futures_util::AsyncReadExt::read(&mut self.0, &mut buffer[range]).await;
        (result, buffer)
    }
    #[cfg(feature = "websocket")]
    fn into_futures(self, _: usize) -> Pin<Box<dyn futures_util::AsyncRead>> {
        Box::pin(self.0)
    }
}
impl<T: futures_util::AsyncWrite + Unpin + 'static> WriteIo for Futures<T> {
    async fn write(&mut self, buffer: Vec<u8>) -> (io::Result<()>, Vec<u8>) {
        let result = futures_util::AsyncWriteExt::write_all(&mut self.0, &buffer).await;
        (result, buffer)
    }
    async fn flush(&mut self) -> io::Result<()> {
        futures_util::AsyncWriteExt::flush(&mut self.0).await
    }
    async fn close(&mut self) -> io::Result<()> {
        futures_util::AsyncWriteExt::close(&mut self.0).await
    }
    #[cfg(feature = "websocket")]
    fn into_futures(self, _: usize) -> Pin<Box<dyn futures_util::AsyncWrite>> {
        Box::pin(self.0)
    }
}
