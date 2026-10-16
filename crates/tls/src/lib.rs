//! TLS 1.3-only configuration with ALPN and bounded handshakes.
#![forbid(unsafe_code)]
pub use rustls;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use std::{io, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct TlsConfig(Arc<rustls::ServerConfig>);
impl TlsConfig {
    pub fn from_pem(certificates: &[u8], key: &[u8], http2: bool) -> io::Result<Self> {
        let certificates = CertificateDer::pem_slice_iter(certificates)
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::other)?;
        let key = PrivateKeyDer::from_pem_slice(key).map_err(io::Error::other)?;
        Self::from_der(certificates, key, http2)
    }
    pub fn from_der(
        certificates: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
        http2: bool,
    ) -> io::Result<Self> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(io::Error::other)?
            .with_no_client_auth()
            .with_single_cert(certificates, key)
            .map_err(io::Error::other)?;
        config.alpn_protocols = if http2 {
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        } else {
            vec![b"http/1.1".to_vec()]
        };
        // TLS 1.3 resumption uses bounded server-side session storage. Early data is disabled.
        config.session_storage = rustls::server::ServerSessionMemoryCache::new(256);
        config.max_early_data_size = 0;
        Ok(Self(Arc::new(config)))
    }
    pub fn rustls(&self) -> &Arc<rustls::ServerConfig> {
        &self.0
    }
    pub async fn accept(
        &self,
        stream: compio::net::TcpStream,
        buffer_bytes: usize,
        deadline: Duration,
    ) -> io::Result<compio::tls::TlsStream<compio::net::TcpStream>> {
        let acceptor = compio::tls::TlsAcceptor::from(self.0.clone());
        let io = compio::io::compat::AsyncStream::with_limits(buffer_bytes, buffer_bytes, stream);
        compio::time::timeout(deadline, acceptor.accept_compat(io))
            .await
            .map_err(|_| {
                io::Error::new(io::ErrorKind::TimedOut, "TLS handshake deadline exceeded")
            })?
    }
}
