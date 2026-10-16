use send_wrapper::SendWrapper;
use std::{
    cell::RefCell,
    future::Future,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
type Task = Pin<Box<dyn Future<Output = ()>>>;
type Callback = Box<dyn FnOnce(Upgraded) -> Task>;
/// One upgraded task per connection, retained within admission and drain accounting.
#[derive(Clone)]
pub struct UpgradeSlot(SendWrapper<Rc<RefCell<Option<Callback>>>>);
impl Default for UpgradeSlot {
    fn default() -> Self {
        Self(SendWrapper::new(Rc::new(RefCell::new(None))))
    }
}
impl UpgradeSlot {
    pub fn register<F, Fut>(&self, callback: F) -> bool
    where
        F: FnOnce(Upgraded) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        let mut slot = self.0.borrow_mut();
        if slot.is_some() {
            return false;
        }
        *slot = Some(Box::new(move |io| Box::pin(callback(io))));
        true
    }
    pub(crate) fn take(&self) -> Option<Callback> {
        self.0.borrow_mut().take()
    }
}
/// Upgraded transport. Bytes read with the HTTP handshake precede socket reads exactly once.
pub struct Upgraded {
    pub(crate) read: Pin<Box<dyn futures_util::AsyncRead>>,
    pub(crate) write: Pin<Box<dyn futures_util::AsyncWrite>>,
    pub(crate) pending: Vec<u8>,
    pub(crate) offset: usize,
    pub(crate) activity: crate::Activity,
}
impl futures_util::AsyncRead for Upgraded {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        if self.offset < self.pending.len() {
            let n = buf.len().min(self.pending.len() - self.offset);
            buf[..n].copy_from_slice(&self.pending[self.offset..self.offset + n]);
            self.offset += n;
            return Poll::Ready(Ok(n));
        }
        let result = self.read.as_mut().poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(n)) if n > 0) {
            self.activity.touch();
        }
        result
    }
}
impl futures_util::AsyncWrite for Upgraded {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = self.write.as_mut().poll_write(cx, buf);
        if matches!(result, Poll::Ready(Ok(n)) if n > 0) {
            self.activity.touch();
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.write.as_mut().poll_flush(cx)
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.write.as_mut().poll_close(cx)
    }
}
