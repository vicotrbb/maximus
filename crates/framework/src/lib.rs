//! Maximus: per-core Linux HTTP services with statically dispatched handlers.
#![forbid(unsafe_code)]
pub use maximus_core::*;
#[cfg(feature = "macros")]
pub use maximus_macros::routes;
#[cfg(feature = "dynamic")]
pub use maximus_router::DynamicRouter;
pub use maximus_runtime::{Backend, Shutdown, WorkerConfig};
#[doc(hidden)]
pub mod __private {
    pub use maximus_router::{matches, validate_path};
}
mod server;
pub use server::{RunningServer, Server};

#[cfg(feature = "tls")]
pub use maximus_tls::TlsConfig;

mod transport;

#[cfg(feature = "cors")]
mod cors;
#[cfg(feature = "cors")]
pub use cors::Cors;
#[cfg(feature = "compression")]
mod compression;
#[cfg(feature = "compression")]
pub use compression::Compression;

#[cfg(feature = "websocket")]
mod websocket;
#[cfg(feature = "websocket")]
pub use websocket::{Message, WebSocket, WebSocketUpgrade};

#[cfg(feature = "opentelemetry")]
mod telemetry;
#[cfg(feature = "opentelemetry")]
pub use telemetry::telemetry;

/// Compile-time checks reject duplicate route shapes and incorrectly typed handlers.
///
/// ```compile_fail
/// use maximus::{routes, Request};
/// async fn handler(_: Request) -> &'static str { "ok" }
/// let _ = routes! { GET "/{x}" => handler, GET "/{y}" => handler };
/// ```
///
/// ```compile_fail
/// use maximus::{routes, Request};
/// async fn handler(_: Request, id: String) -> String { id }
/// let _ = routes! { GET "/{id:u64}" => handler };
/// ```
///
/// ```compile_fail
/// use maximus::{routes, Request};
/// async fn handler(_: Request) -> &'static str { "ok" }
/// let _ = routes! { GOT "/" => handler };
/// ```
#[cfg(feature = "macros")]
#[doc(hidden)]
pub struct RouteCompileChecks;
