//! Instrumentation only: counts allocations through the system allocator.
use maximus::{Body, Handler, Request, Server};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
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
fn start() {
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Relaxed);
}
fn end(name: &str, count: usize) {
    ACTIVE.store(false, Ordering::Relaxed);
    println!(
        "{name},{count},{:.4},{:.4}",
        ALLOCS.load(Ordering::Relaxed) as f64 / count as f64,
        BYTES.load(Ordering::Relaxed) as f64 / count as f64
    );
}
fn main() {
    use std::io::{Read, Write};
    const N: usize = 10000;
    println!("path,requests,allocations_per_request,allocated_bytes_per_request");
    maximus_runtime::WorkerConfig::default()
        .build(0)
        .unwrap()
        .block_on(async {
            async fn body(_: Request) -> Body {
                "Hello, World!".into()
            }
            let handler = maximus::routes! { GET "/" => body };
            start();
            for _ in 0..N {
                black_box(
                    handler
                        .handle(Request::new(maximus::http::Request::new(Body::empty())))
                        .await,
                );
            }
            end("static-route-body", N);
            assert_eq!(
                ALLOCS.load(Ordering::Relaxed),
                0,
                "static route and borrowed body must stay allocation-free"
            );
            let handler = |_: Request| async { "Hello, World!" };
            start();
            for _ in 0..N {
                black_box(
                    handler
                        .handle(Request::new(maximus::http::Request::new(Body::empty())))
                        .await,
                );
            }
            end("static-text-content-type", N);
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
    for index in 0..N + 100 {
        if index == 100 {
            start();
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
    end("http1-keepalive-complete", N);
    server.shutdown().unwrap();
}
