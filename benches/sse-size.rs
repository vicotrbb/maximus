//! Minimal SSE application used only for linked-footprint comparisons.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    maximus::Server::new("127.0.0.1:8080".parse()?)
        .start(|| maximus::routes! { GET "/events" => events })?
        .wait()?;
    Ok(())
}
async fn events(_: maximus::Request) -> impl maximus::IntoResponse {
    maximus::Sse::new(futures_util::stream::iter([Ok(maximus::Event::new(
        "hello",
    ))]))
}
