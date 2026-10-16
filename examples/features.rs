//! Runnable protocol and optional-feature demonstration. Enter on stdin drains workers.
use futures_util::StreamExt;
use http_body_util::BodyExt;
use maximus::{
    Body, Bytes, Compression, Cors, Event, IntoResponse, Json, Request, Response, Server, Sse,
    TlsConfig, WebSocketUpgrade, routes,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or("127.0.0.1:8080".into())
        .parse()?;
    let mut server = Server::new(address).limits(maximus::Limits {
        memory_bytes: 2 * 1024 * 1024 * 1024,
        ..Default::default()
    });
    if let Ok(cert) = std::env::var("MAXIMUS_CERT") {
        let tls = TlsConfig::from_pem(
            &std::fs::read(cert)?,
            &std::fs::read(std::env::var("MAXIMUS_KEY")?)?,
            true,
        )?;
        server = server.tls(tls.clone()).http3(tls);
    }
    if std::env::var_os("MAXIMUS_H2C").is_some() {
        server = server.http2_prior_knowledge(true);
    }
    let server = server.start(|| {
        let routes = routes! {
            GET "/plaintext" => plain,
            GET "/cookies" => cookies,
            GET "/users/{id:u64}" => user,
            POST "/echo" => echo,
            POST "/json" => json,
            POST "/upload" => upload,
            GET "/events" => events,
            GET "/large" => large,
            GET "/ws" => websocket,
        };
        Cors::new(
            ["https://example.com"],
            [maximus::Method::GET, maximus::Method::POST],
            ["content-type"],
            false,
        )
        .unwrap()
        .wrap(Compression::default().wrap(routes))
    })?;
    println!("listening {}", server.local_addr());
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    server.shutdown()?;
    Ok(())
}
async fn plain(_: Request) -> &'static str {
    "Hello, World!"
}
async fn cookies(request: Request) -> Result<String, maximus::Error> {
    request.header("cookie")
}
async fn user(_: Request, id: u64) -> String {
    id.to_string()
}
async fn echo(request: Request) -> Result<Bytes, maximus::Error> {
    Ok(request.into_body().collect().await?.to_bytes())
}
async fn json(request: Request) -> Result<Json<serde_json::Value>, maximus::Error> {
    Ok(Json(request.json().await?))
}
async fn upload(request: Request) -> Result<String, maximus::Error> {
    let mut multipart = request.multipart(1024 * 1024, 512 * 1024)?;
    let mut bytes = 0;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| maximus::Error::bad_request("invalid upload"))?
    {
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|_| maximus::Error::bad_request("invalid field"))?
        {
            bytes += chunk.len();
        }
    }
    Ok(bytes.to_string())
}
async fn events(_: Request) -> Response {
    Sse::new(futures_util::stream::iter([
        Ok(Event::new("first").id("1").unwrap()),
        Ok(Event::new("second")),
    ]))
    .into_response()
}
async fn large(_: Request) -> Body {
    "x".repeat(4096).into()
}
async fn websocket(request: Request) -> Result<Response, maximus::Error> {
    Ok(
        WebSocketUpgrade::new(request)?.on_upgrade(|mut socket| async move {
            while let Some(Ok(message)) = socket.next().await {
                if message.is_close() {
                    break;
                }
                if (message.is_text() || message.is_binary()) && socket.send(message).await.is_err()
                {
                    break;
                }
            }
        }),
    )
}
