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
        let io = ciphertext_io(stream, buffer_bytes);
        compio::time::timeout(deadline, acceptor.accept_compat(io))
            .await
            .map_err(|_| {
                io::Error::new(io::ErrorKind::TimedOut, "TLS handshake deadline exceeded")
            })?
    }
}

fn ciphertext_io<S: compio::io::util::Splittable>(
    stream: S,
    buffer_bytes: usize,
) -> compio::io::compat::AsyncStream<S> {
    // Ciphertext includes record overhead beyond the configured plaintext batch.
    compio::io::compat::AsyncStream::with_limits(
        buffer_bytes,
        buffer_bytes.saturating_mul(2),
        stream,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use compio::{BufResult, buf::IoBuf};
    use futures_util::AsyncWriteExt;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Default)]
    struct Writes {
        sizes: Vec<usize>,
        bytes: Vec<u8>,
    }
    struct Writer(Rc<RefCell<Writes>>);
    impl compio::io::AsyncWrite for Writer {
        async fn write<B: IoBuf>(&mut self, bytes: B) -> BufResult<usize, B> {
            let mut writes = self.0.borrow_mut();
            writes.sizes.push(bytes.buf_len());
            writes.bytes.extend_from_slice(bytes.as_init());
            BufResult(Ok(bytes.buf_len()), bytes)
        }
        async fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
        async fn shutdown(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn ciphertext_batches_coalesce_within_the_bounded_adapter() {
        compio::runtime::Runtime::new().unwrap().block_on(async {
            for (size, expected_writes) in [(1050, vec![1050]), (2049, vec![2048, 1])] {
                let writes = Rc::new(RefCell::new(Writes::default()));
                let mut io = Box::pin(ciphertext_io(
                    (b"".as_slice(), Writer(writes.clone())),
                    1024,
                ));
                let payload = vec![b'x'; size];
                io.write_all(&payload).await.unwrap();
                io.flush().await.unwrap();
                let writes = writes.borrow();
                assert_eq!(writes.sizes, expected_writes);
                assert!(writes.sizes.iter().all(|&n| n <= 2048));
                assert_eq!(writes.bytes, payload);
            }
        });
    }
}
