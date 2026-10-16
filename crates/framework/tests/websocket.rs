#![cfg(feature = "websocket")]
use futures_util::StreamExt;
use maximus::{Request, Server, WebSocketUpgrade};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};
#[test]
fn websocket_handshake_masked_echo_and_shutdown() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |request: Request| async move {
                WebSocketUpgrade::new(request)
                    .and_then(|upgrade| upgrade.protocols(&["chat"]))
                    .map(|upgrade| {
                        upgrade.limits(8, 8).on_upgrade(|mut socket| async move {
                            while let Some(Ok(message)) = socket.next().await {
                                if message.is_close() {
                                    let _ = socket.close(None).await;
                                    break;
                                }
                                if socket.send(message).await.is_err() {
                                    break;
                                }
                            }
                        })
                    })
            }
        })
        .unwrap();
    let stream = TcpStream::connect(server.local_addr()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    use async_tungstenite::tungstenite::client::IntoClientRequest;
    let mut request = format!("ws://{}/", server.local_addr())
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("sec-websocket-protocol", "other, chat".parse().unwrap());
    let (mut websocket, response) =
        async_tungstenite::tungstenite::client(request, stream).unwrap();
    assert_eq!(response.status(), 101);
    assert_eq!(response.headers()["sec-websocket-protocol"], "chat");
    websocket
        .send(async_tungstenite::tungstenite::Message::Text(
            "hello".into(),
        ))
        .unwrap();
    assert_eq!(websocket.read().unwrap().into_text().unwrap(), "hello");
    websocket
        .send(async_tungstenite::tungstenite::Message::Binary(
            vec![42; 9].into(),
        ))
        .unwrap();
    assert!(
        websocket.read().is_err(),
        "configured frame/message limit must close the connection"
    );
    let mut bad = TcpStream::connect(server.local_addr()).unwrap();
    bad.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    bad.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    bad.read_to_string(&mut response).unwrap();
    assert!(response.contains("400 Bad Request"), "{response}");
    server.shutdown().unwrap();
}

#[test]
fn upgraded_connection_keeps_its_admission_slot_and_drain_deadline() {
    let limits = maximus::Limits {
        connections: 1,
        drain_timeout: Duration::from_millis(40),
        ..Default::default()
    };
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .limits(limits)
        .start(|| {
            |request: Request| async move {
                WebSocketUpgrade::new(request)
                    .map(|upgrade| upgrade.on_upgrade(|_socket| futures_util::future::pending()))
            }
        })
        .unwrap();
    let stream = TcpStream::connect(server.local_addr()).unwrap();
    let (_websocket, _) =
        async_tungstenite::tungstenite::client(format!("ws://{}/", server.local_addr()), stream)
            .unwrap();
    let mut waiting = TcpStream::connect(server.local_addr()).unwrap();
    waiting
        .set_read_timeout(Some(Duration::from_millis(80)))
        .unwrap();
    waiting
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let error = waiting.read(&mut [0u8; 1]).unwrap_err();
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    let start = std::time::Instant::now();
    server.shutdown().unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
}
