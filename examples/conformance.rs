//! HTTP/2 fixture: consume requests, holding `/half-closed` responses open.
use http_body_util::BodyExt;
use maximus::{Request, Server, TlsConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or("127.0.0.1:8080".into())
        .parse()?;
    let mut server = Server::new(address);
    if let Ok(cert) = std::env::var("MAXIMUS_CERT") {
        server = server.tls(TlsConfig::from_pem(
            &std::fs::read(cert)?,
            &std::fs::read(std::env::var("MAXIMUS_KEY")?)?,
            true,
        )?);
    } else {
        server = server.http2_prior_knowledge(true);
    }
    let running = server.start(|| {
        |request: Request| async {
            let hold_response = request.uri().path() == "/half-closed";
            request.into_body().collect().await?;
            if hold_response {
                futures_util::future::pending::<()>().await;
            }
            Ok::<_, maximus::Error>("Hello, World!")
        }
    })?;
    println!("listening {}", running.local_addr());
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    running.shutdown()?;
    Ok(())
}
