//! Instrumentation only: counts allocations through the system allocator.
use maximus::{Body, DynamicRouter, Handler, Request, Server};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};
struct Count;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
// SAFETY: Every allocation operation is forwarded unchanged to System. Counters
// are atomic and never dereference, retain or modify the returned allocation.
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        // SAFETY: The caller supplies the GlobalAlloc layout contract, forwarded unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: This is the same pointer/layout pair supplied to this allocator by its caller.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(size, Ordering::Relaxed);
        }
        // SAFETY: System owns this pointer and receives the caller's unchanged allocation contract.
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Count = Count;
fn start() -> Instant {
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Relaxed);
    Instant::now()
}
fn end(name: &str, count: usize, started: Instant) {
    ACTIVE.store(false, Ordering::Relaxed);
    println!(
        "{name},{count},{:.4},{:.4},{:.2}",
        ALLOCS.load(Ordering::Relaxed) as f64 / count as f64,
        BYTES.load(Ordering::Relaxed) as f64 / count as f64,
        started.elapsed().as_nanos() as f64 / count as f64
    );
}
fn main() {
    use std::io::{Read, Write};
    const N: usize = 10000;
    println!("path,requests,allocations_per_request,allocated_bytes_per_request,ns_per_request");
    maximus_runtime::WorkerConfig::default()
        .build(0)
        .unwrap()
        .block_on(async {
            async fn body(_: Request) -> Body {
                "Hello, World!".into()
            }
            let handler = maximus::routes! { GET "/" => body };
            let started = start();
            for _ in 0..N {
                black_box(
                    handler
                        .handle(Request::new(maximus::http::Request::new(Body::empty())))
                        .await,
                );
            }
            end("static-route-body", N, started);
            assert_eq!(
                ALLOCS.load(Ordering::Relaxed),
                0,
                "static route and borrowed body must stay allocation-free"
            );
            let handler = |_: Request| async { "Hello, World!" };
            let started = start();
            for _ in 0..N {
                black_box(
                    handler
                        .handle(Request::new(maximus::http::Request::new(Body::empty())))
                        .await,
                );
            }
            end("static-text-content-type", N, started);
            dynamic_allocations(N).await;
        });
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .limits(maximus::Limits {
            requests_per_connection: usize::MAX,
            ..Default::default()
        })
        .start(|| |_: Request| async { "Hello, World!" })
        .unwrap();
    let mut socket = std::net::TcpStream::connect(server.local_addr()).unwrap();
    socket.set_nodelay(true).unwrap();
    let mut buffer = [0u8; 1024];
    let mut started = Instant::now();
    for index in 0..N + 100 {
        if index == 100 {
            started = start();
        }
        socket
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut received = 0;
        while !buffer[..received].ends_with(b"Hello, World!") {
            let n = socket.read(&mut buffer[received..]).unwrap();
            assert!(n > 0);
            received += n;
        }
        assert!(buffer[..received].starts_with(b"HTTP/1.1 200"));
    }
    end("http1-keepalive-complete", N, started);
    drop(socket);
    server.shutdown().unwrap();
    http2_allocations(N);
}

/// Sequential cleartext H2 with a stack-only wire client. Counts process-wide
/// allocations, including server scheduling; excludes handshake and warmup.
fn http2_allocations(count: usize) {
    use std::io::{Read, Write};
    use std::time::Duration;
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .http2_prior_knowledge(true)
        .limits(maximus::Limits {
            requests_per_connection: usize::MAX,
            ..Default::default()
        })
        .start(|| |_: Request| async { "Hello, World!" })
        .unwrap();
    let mut socket = std::net::TcpStream::connect(server.local_addr()).unwrap();
    socket.set_nodelay(true).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
        .unwrap();
    socket.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).unwrap();
    // Plenty of connection credit for every measured 13-byte response. Each
    // stream retains its default 65,535-byte window; no timed WINDOW_UPDATEs.
    let credit = u32::try_from((count + 100) * 13).unwrap();
    socket.write_all(&[0, 0, 4, 8, 0, 0, 0, 0, 0]).unwrap();
    socket.write_all(&credit.to_be_bytes()).unwrap();
    let mut payload = [0u8; 16384];
    let mut receive = |socket: &mut std::net::TcpStream| {
        let mut header = [0u8; 9];
        socket.read_exact(&mut header).unwrap();
        let size = u32::from_be_bytes([0, header[0], header[1], header[2]]) as usize;
        assert!(size <= payload.len());
        socket.read_exact(&mut payload[..size]).unwrap();
        let stream = u32::from_be_bytes(header[5..].try_into().unwrap());
        if header[3] == 0 && size != 0 {
            assert_eq!(&payload[..size], b"Hello, World!");
        }
        if header[3] == 1 {
            let status = usize::from(payload[0] == 0x20); // optional table-size reset
            assert!(size > status);
            assert_eq!(payload[status], 0x88, "HPACK static :status 200");
        }
        (header[3], header[4], stream, size)
    };
    loop {
        let (kind, flags, stream, size) = receive(&mut socket);
        assert_eq!(stream, 0);
        if kind == 8 {
            assert_eq!(size, 4);
            continue;
        }
        assert_eq!(kind, 4, "unexpected handshake frame");
        if flags == 0 {
            socket.write_all(&[0, 0, 0, 4, 1, 0, 0, 0, 0]).unwrap();
        } else {
            break;
        }
    }
    let mut started = Instant::now();
    for index in 0..count + 100 {
        if index == 100 {
            started = start();
        }
        let id = u32::try_from(index * 2 + 1).unwrap();
        let mut request = [0u8; 23];
        request[..9].copy_from_slice(&[0, 0, 14, 1, 5, 0, 0, 0, 0]);
        request[5..9].copy_from_slice(&id.to_be_bytes());
        request[9..].copy_from_slice(b"\x82\x86\x84\x01\x09localhost");
        socket.write_all(&request).unwrap();
        let mut bytes = 0;
        let mut headers = false;
        loop {
            let (kind, flags, stream, size) = receive(&mut socket);
            assert_eq!(stream, id);
            assert!(kind == 0 || kind == 1, "unexpected response frame {kind}");
            if kind == 1 {
                headers = true;
            } else {
                bytes += size;
            }
            if flags & 1 != 0 {
                break;
            }
        }
        assert!(headers);
        assert_eq!(bytes, 13);
    }
    end("http2-keepalive-complete", count, started);
    drop(socket);
    server.shutdown().unwrap();
}

async fn dynamic_allocations(count: usize) {
    let started = start();
    for _ in 0..100 {
        let mut router = DynamicRouter::new(64);
        for id in 0..32 {
            for method in ["GET", "POST"] {
                router
                    .add(
                        method,
                        &format!("/items/{id}/{{value}}"),
                        |_: Request| async { Body::empty() },
                    )
                    .unwrap();
            }
        }
        black_box(router);
    }
    end("dynamic-register-64", 100, started);
    let mut router = DynamicRouter::new(2);
    for method in ["GET", "POST"] {
        router
            .add(method, "/", |_: Request| async { Body::empty() })
            .unwrap();
    }
    for method in [
        maximus::Method::GET,
        maximus::Method::POST,
        maximus::Method::PATCH,
    ] {
        let label = format!("dynamic-{method}");
        let started = start();
        for _ in 0..count {
            let mut request = maximus::http::Request::new(Body::empty());
            *request.method_mut() = method.clone();
            let response = router.handle(Request::new(request)).await;
            assert_eq!(
                response.status(),
                if method == maximus::Method::PATCH {
                    maximus::StatusCode::METHOD_NOT_ALLOWED
                } else {
                    maximus::StatusCode::OK
                }
            );
            black_box(response);
        }
        end(&label, count, started);
    }
}
