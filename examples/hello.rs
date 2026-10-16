use maximus::{Backend, Request, Server, WorkerConfig, routes};
async fn index(_: Request) -> &'static str {
    "Hello from Maximus!\n"
}
async fn user(_: Request, id: u64) -> String {
    format!("user {id}\n")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".into())
        .parse()?;
    let backend = match std::env::var("MAXIMUS_BACKEND").as_deref() {
        Ok("epoll") => Backend::Epoll,
        Ok("uring") => Backend::IoUring,
        Ok("auto") | Err(_) => Backend::Auto,
        Ok(_) => return Err("MAXIMUS_BACKEND must be auto, epoll or uring".into()),
    };
    let workers = std::env::var("MAXIMUS_WORKERS")
        .unwrap_or_else(|_| "1".into())
        .parse()?;
    let server = Server::new(address)
        .workers(WorkerConfig {
            workers,
            backend,
            ..Default::default()
        })
        .start(|| routes! { GET "/" => index, GET "/users/{id:u64}" => user })?;
    println!(
        "Maximus listening on http://{}; press Enter to drain and stop",
        server.local_addr()
    );
    std::io::stdin().read_line(&mut String::new())?;
    server.shutdown()?;
    Ok(())
}
