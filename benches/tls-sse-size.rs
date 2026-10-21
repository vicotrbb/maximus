//! SSE with live TLS/H2 configuration, used only for native linked-footprint checks.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let certificate = std::fs::read(args.next().ok_or("certificate path required")?)?;
    let key = std::fs::read(args.next().ok_or("key path required")?)?;
    maximus::Server::new("127.0.0.1:8080".parse()?)
        .tls(maximus::TlsConfig::from_pem(&certificate, &key, true)?)
        .start(|| maximus::routes! { GET "/events" => events })?
        .wait()?;
    Ok(())
}
async fn events(_: maximus::Request) -> impl maximus::IntoResponse {
    maximus::Sse::new(futures_util::stream::iter([Ok(maximus::Event::new(
        "hello",
    ))]))
}
