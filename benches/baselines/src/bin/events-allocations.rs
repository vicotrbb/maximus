//! SSE allocation diagnostic; timing is reported separately by the events probe.
use http_body_util::BodyExt;
use maximus::IntoResponse;
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
    maximus_runtime::WorkerConfig::default()
        .build(0)
        .unwrap()
        .block_on(async {
            println!(
                "case,operations,allocations_per_event,requested_bytes_per_event,ns_per_event"
            );
            for (name, data) in [
                ("empty", ""),
                ("single", "hello"),
                ("multiline", "one\ntwo\nthree"),
                ("crlf", "one\r\ntwo\rthree\n"),
                ("fields", "hello"),
            ] {
                const N: usize = 10_000;
                let started = start();
                for _ in 0..N {
                    let mut event = maximus::Event::new(black_box(data));
                    if name == "fields" {
                        event = event
                            .id("id")
                            .unwrap()
                            .event("event")
                            .unwrap()
                            .retry(std::time::Duration::from_millis(1234));
                    }
                    let response =
                        maximus::Sse::new(futures_util::stream::iter([Ok(event)])).into_response();
                    black_box(response.into_body().collect().await.unwrap().to_bytes());
                }
                end(name, N, started);
            }
        });
}
