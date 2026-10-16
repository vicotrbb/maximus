//! HTTP/2 multiplexing, HPACK, bounded flow-control and connection drain.
#![forbid(unsafe_code)]
use futures_util::{FutureExt, pin_mut, select_biased};
use maximus_core::{Handler, IntoResponse, Limits, StatusCode};
use maximus_http1::{Activity, Executor, Timer, adapt, dispatch};
use maximus_runtime::Shutdown;
use std::{cell::Cell, convert::Infallible, rc::Rc};

pub async fn serve<S, H>(
    stream: S,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> Result<(), hyper::Error>
where
    S: futures_util::AsyncRead + futures_util::AsyncWrite + 'static,
    H: Handler,
{
    let count = Rc::new(Cell::new(0usize));
    let exhausted = Shutdown::default();
    let service_stop = exhausted.clone();
    let service_limits = limits.clone();
    let service = hyper::service::service_fn(move |request| {
        let handler = handler.clone();
        let limits = service_limits.clone();
        let count = count.clone();
        let exhausted = service_stop.clone();
        async move {
            if count.get() >= limits.requests_per_connection {
                return Ok::<_, Infallible>(StatusCode::SERVICE_UNAVAILABLE.into_response());
            }
            count.set(count.get() + 1);
            if count.get() == limits.requests_per_connection {
                exhausted.cancel();
            }
            Ok::<_, Infallible>(dispatch(request, handler, limits).await)
        }
    });
    let mut builder = hyper::server::conn::http2::Builder::new(Executor);
    builder
        .timer(Timer)
        .max_concurrent_streams(limits.streams)
        .initial_stream_window_size(limits.stream_window_bytes)
        .initial_connection_window_size(limits.connection_window_bytes)
        .max_header_list_size(limits.header_bytes.min(u32::MAX as usize) as u32)
        .max_send_buf_size(limits.io_buffer_bytes)
        .max_pending_accept_reset_streams(limits.streams as usize)
        .max_local_error_reset_streams(limits.streams as usize)
        .keep_alive_interval(limits.header_timeout)
        .keep_alive_timeout(limits.header_timeout);
    let (stream, activity) = Activity::track(stream);
    let connection = builder.serve_connection(adapt(stream), service);
    let idle = activity.idle(limits.idle_timeout).fuse();
    pin_mut!(idle);
    pin_mut!(connection);
    let stop = shutdown.cancelled().fuse();
    let exhausted = exhausted.cancelled().fuse();
    pin_mut!(stop, exhausted);
    select_biased! {
        _ = idle => return Ok(()),
        _ = stop => {},
        _ = exhausted => {},
        result = connection.as_mut().fuse() => return result,
    }
    connection.as_mut().graceful_shutdown();
    match compio::time::timeout(limits.drain_timeout, connection).await {
        Ok(result) => result,
        Err(_) => Ok(()),
    }
}
