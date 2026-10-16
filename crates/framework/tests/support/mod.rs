//! Independent Hyper client adapter, intentionally outside the product graph.
use std::future::Future;
use tokio_util::compat::FuturesAsyncReadCompatExt;

#[derive(Clone)]
pub struct Executor;
impl<F: Future<Output = ()> + 'static> hyper::rt::Executor<F> for Executor {
    fn execute(&self, future: F) {
        compio::runtime::spawn(future).detach();
    }
}

pub fn adapt<S: futures_util::AsyncRead + futures_util::AsyncWrite + 'static>(
    stream: S,
) -> impl hyper::rt::Read + hyper::rt::Write + Unpin {
    hyper_util::rt::TokioIo::new(Box::pin(stream).compat())
}
