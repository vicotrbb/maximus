use futures_util::{AsyncRead, AsyncWrite};
use std::{
    cell::Cell,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

/// Connection-local idle deadline. I/O progress extends it without allocating a timer per operation.
#[derive(Clone)]
pub struct Activity(Rc<Cell<Instant>>);
impl Activity {
    pub(crate) fn new() -> Self {
        Self(Rc::new(Cell::new(Instant::now())))
    }
    pub(crate) fn touch(&self) {
        self.0.set(Instant::now());
    }
    pub fn track<S>(stream: S) -> (Tracked<S>, Self) {
        let activity = Self(Rc::new(Cell::new(Instant::now())));
        (
            Tracked {
                stream: Box::pin(stream),
                activity: activity.clone(),
            },
            activity,
        )
    }
    pub async fn idle(&self, duration: Duration) {
        loop {
            let deadline = self.0.get() + duration;
            compio::time::sleep_until(deadline).await;
            if Instant::now().duration_since(self.0.get()) >= duration {
                return;
            }
        }
    }
    fn progress(&self, result: &Poll<io::Result<usize>>) {
        if matches!(result, Poll::Ready(Ok(n)) if *n > 0) {
            self.0.set(Instant::now());
        }
    }
}
pub struct Tracked<S> {
    stream: Pin<Box<S>>,
    activity: Activity,
}
impl<S: AsyncRead> AsyncRead for Tracked<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let result = self.stream.as_mut().poll_read(cx, buf);
        self.activity.progress(&result);
        result
    }
}
impl<S: AsyncWrite> AsyncWrite for Tracked<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = self.stream.as_mut().poll_write(cx, buf);
        self.activity.progress(&result);
        result
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let result = self.stream.as_mut().poll_write_vectored(cx, bufs);
        self.activity.progress(&result);
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.stream.as_mut().poll_flush(cx)
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.stream.as_mut().poll_close(cx)
    }
}
