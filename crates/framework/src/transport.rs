use maximus_core::{Handler, Limits};
use maximus_runtime::Shutdown;
use std::{io, rc::Rc};
#[derive(Clone, Default)]
pub(crate) struct Transport {
    #[cfg(feature = "tls")]
    pub tls: Option<maximus_tls::TlsConfig>,
    #[cfg(feature = "http2")]
    pub http2_prior_knowledge: bool,
}
impl Transport {
    pub fn capacity(&self, limits: &Limits) -> usize {
        #[cfg(feature = "tls")]
        let tls = self.tls.is_some();
        #[cfg(not(feature = "tls"))]
        let tls = false;
        #[cfg(feature = "http2")]
        let multiplexed = self.http2_prior_knowledge || {
            #[cfg(feature = "tls")]
            {
                self.tls
                    .as_ref()
                    .is_some_and(|config| config.rustls().alpn_protocols.iter().any(|p| p == b"h2"))
            }
            #[cfg(not(feature = "tls"))]
            {
                false
            }
        };
        #[cfg(not(feature = "http2"))]
        let multiplexed = false;
        limits.protocol_capacity(multiplexed, tls)
    }
    pub fn validate(&self) -> io::Result<()> {
        #[cfg(all(feature = "tls", not(feature = "http2")))]
        if self
            .tls
            .as_ref()
            .is_some_and(|tls| tls.rustls().alpn_protocols.iter().any(|p| p == b"h2"))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "h2 ALPN requires the http2 feature",
            ));
        }
        Ok(())
    }
    pub async fn serve<H: Handler>(
        &self,
        stream: compio::net::TcpStream,
        handler: Rc<H>,
        limits: Rc<Limits>,
        shutdown: Shutdown,
    ) {
        #[cfg(feature = "tls")]
        if let Some(tls) = &self.tls {
            if let Ok(stream) = tls
                .accept(stream, limits.io_buffer_bytes, limits.header_timeout)
                .await
            {
                #[cfg(feature = "http2")]
                if stream.negotiated_alpn().as_deref() == Some(b"h2".as_slice()) {
                    let _ = maximus_http2::serve(stream, handler, limits, shutdown).await;
                    return;
                }
                let _ = maximus_http1::serve_futures(stream, handler, limits, shutdown).await;
            }
            return;
        }
        #[cfg(feature = "http2")]
        if self.http2_prior_knowledge {
            let stream = compio::io::compat::AsyncStream::with_limits(
                limits.io_buffer_bytes,
                limits.io_buffer_bytes,
                stream,
            );
            let _ = maximus_http2::serve(stream, handler, limits, shutdown).await;
            return;
        }
        let _ = maximus_http1::serve(stream, handler, limits, shutdown).await;
    }
}
