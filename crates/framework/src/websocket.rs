pub use async_tungstenite::tungstenite::Message;
use async_tungstenite::{
    WebSocketStream,
    tungstenite::{
        handshake::server::create_response_with_body,
        protocol::{Role, WebSocketConfig},
    },
};
use maximus_core::{Body, Error, IntoResponse, Request, Response, Result, StatusCode};
use std::future::Future;
pub type WebSocket = WebSocketStream<maximus_http1::Upgraded>;

pub struct WebSocketUpgrade {
    response: Response,
    slot: maximus_http1::UpgradeSlot,
    config: WebSocketConfig,
    protocols: Vec<String>,
}
impl WebSocketUpgrade {
    pub fn new(request: Request) -> Result<Self> {
        if request.version() != maximus_core::http::Version::HTTP_11 {
            return Err(Error::bad_request("WebSocket upgrade requires HTTP/1.1"));
        }
        for header in ["sec-websocket-key", "sec-websocket-version", "upgrade"] {
            if request.headers().get_all(header).iter().count() != 1 {
                return Err(Error::bad_request("missing or duplicate WebSocket header"));
            }
        }
        let response = create_response_with_body(&*request, Body::empty)
            .map_err(|_| Error::bad_request("invalid WebSocket handshake"))?;
        let slot = request
            .extensions()
            .get::<maximus_http1::UpgradeSlot>()
            .cloned()
            .ok_or_else(|| Error::bad_request("upgrade unavailable on this transport"))?;
        let protocols = request
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(',').map(|s| s.trim().to_owned()).collect())
            .unwrap_or_default();
        let config = WebSocketConfig::default()
            .read_buffer_size(4096)
            .write_buffer_size(16384)
            .max_write_buffer_size(65536)
            .max_message_size(Some(1024 * 1024))
            .max_frame_size(Some(65536));
        Ok(Self {
            response,
            slot,
            config,
            protocols,
        })
    }
    pub fn limits(mut self, message_bytes: usize, frame_bytes: usize) -> Self {
        self.config = self
            .config
            .max_message_size(Some(message_bytes))
            .max_frame_size(Some(frame_bytes));
        self
    }
    pub fn protocols(mut self, supported: &[&str]) -> Result<Self> {
        for protocol in supported {
            if maximus_core::Method::from_bytes(protocol.as_bytes()).is_err() {
                return Err(Error::bad_request("invalid WebSocket subprotocol"));
            }
            if self.protocols.iter().any(|offered| offered == protocol) {
                self.response.headers_mut().insert(
                    "sec-websocket-protocol",
                    protocol
                        .parse()
                        .map_err(|_| Error::bad_request("invalid WebSocket subprotocol"))?,
                );
                break;
            }
        }
        Ok(self)
    }
    pub fn on_upgrade<F, Fut>(self, handler: F) -> Response
    where
        F: FnOnce(WebSocket) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        if self.slot.register(move |upgraded| async move {
            let socket =
                WebSocketStream::from_raw_socket(upgraded, Role::Server, Some(self.config)).await;
            handler(socket).await;
        }) {
            self.response
        } else {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
