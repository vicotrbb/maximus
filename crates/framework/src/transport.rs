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

#[cfg(all(test, feature = "tls"))]
mod tests {
    use super::*;

    #[test]
    fn tcp_tls_admission_reserves_ciphertext_buffers_at_memory_boundary() {
        for (h2, per_connection) in [(false, 147456), (true, 294912)]
            .into_iter()
            .filter(|&(h2, _)| !h2 || cfg!(feature = "http2"))
        {
            let rcgen::CertifiedKey { cert, signing_key } =
                rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
            let transport = Transport {
                tls: Some(
                    maximus_tls::TlsConfig::from_der(
                        vec![cert.der().clone()],
                        signing_key.serialize_der().try_into().unwrap(),
                        h2,
                    )
                    .unwrap(),
                ),
                ..Transport::default()
            };
            let mut limits = Limits {
                connections: 10,
                streams: 2,
                body_bytes: 0,
                header_bytes: 8192,
                io_buffer_bytes: 16384,
                connection_window_bytes: 65536,
                ..Limits::default()
            };
            // Includes doubled TCP TLS adapters, independent of the generic QUIC estimate.
            for (memory, admitted) in [
                (per_connection - 1, 0),
                (per_connection, 1),
                (2 * per_connection - 1, 1),
                (2 * per_connection, 2),
            ] {
                limits.memory_bytes = memory;
                assert_eq!(transport.capacity(&limits), admitted);
            }
            limits.io_buffer_bytes = usize::MAX;
            limits.memory_bytes = usize::MAX - 1;
            assert_eq!(transport.capacity(&limits), 0);
            limits.memory_bytes = usize::MAX;
            assert_eq!(transport.capacity(&limits), 1);
        }
    }
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
        #[cfg(feature = "tls")]
        if tls {
            // Only TCP TLS uses the expanded Compio ciphertext adapters.
            let mut budget = limits.clone();
            budget.io_buffer_bytes = budget.io_buffer_bytes.saturating_mul(2);
            return budget.protocol_capacity(multiplexed, true);
        }
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
                    let _ = maximus_http2::serve_futures(stream, handler, limits, shutdown).await;
                    return;
                }
                let _ = maximus_http1::serve_futures(stream, handler, limits, shutdown).await;
            }
            return;
        }
        #[cfg(feature = "http2")]
        if self.http2_prior_knowledge {
            let _ = maximus_http2::serve(stream, handler, limits, shutdown).await;
            return;
        }
        let _ = maximus_http1::serve(stream, handler, limits, shutdown).await;
    }
}
