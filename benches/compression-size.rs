//! Minimal gzip application used only for linked-footprint comparisons.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    maximus::Server::new("127.0.0.1:8080".parse()?)
        .start(|| {
            maximus::Compression::default()
                .wrap(|_: maximus::Request| async { "hello".repeat(300) })
        })?
        .wait()?;
    Ok(())
}
