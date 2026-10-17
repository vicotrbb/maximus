# Maximus

A modular Rust HTTP server for Linux, with one local executor per worker, io_uring and epoll, statically dispatched routes, and optional TLS, HTTP/2 and HTTP/3.

Maximus owns HTTP/1.1 framing, HTTP/2 and HPACK, worker lifecycle, admission limits, routing and framework policy. Compio supplies completion I/O and local execution; Rustls and h3/Quinn supply TLS and HTTP/3. Plain TCP uses Compio's owned buffers directly; TLS and WebSocket transports retain explicit futures I/O integration. Product builds do not depend on Hyper, hyper-util, tokio-util or the h2 engine. The optional h3 dependency still uses Tokio utilities transitively; Maximus does not start a Tokio executor.

## Run

Requires Rust 1.96 or newer. Linux is the deployment target; macOS supports development with its native poller.

```sh
git clone https://github.com/vicotrbb/maximus.git
cd maximus
cargo run --release -p maximus --example hello -- 127.0.0.1:8080
# In another terminal:
curl http://127.0.0.1:8080/users/42
# Press Enter in the server terminal to drain and stop.
```

Use the local library:

```toml
[dependencies]
maximus = { path = "path/to/maximus/crates/framework" }
```

```rust
use maximus::{routes, Request, Server};

async fn index(_: Request) -> &'static str { "Hello, World!" }
async fn user(_: Request, id: u64) -> String { id.to_string() }

fn main() -> std::io::Result<()> {
    let server = Server::new("127.0.0.1:8080".parse().unwrap())
        .start(|| routes! { GET "/" => index, GET "/users/{id:u64}" => user })?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    server.shutdown()
}
```

The factory runs on each worker. Handlers, futures and typed application state can be `!Send`. Configure `WorkerConfig` to choose worker count, distinct allowed CPUs, backend and submission queue depth. Explicit io_uring fails if unavailable; `Auto` can fall back. Bind happens before workers start. A worker failure cancels its siblings. Dropping `RunningServer` drains to the deadline and joins workers.

## Features

Only `macros` is enabled by default. Features are additive; `--no-default-features` supports a plain asynchronous closure without routing macros.

| Feature | Capability | Protocol compatibility |
| --- | --- | --- |
| `macros` | `routes!`, typed captures, compile-time declaration checks | All |
| `dynamic` | Bounded runtime route registration | All |
| `json` | `Request::json`, `Json<T>` | All |
| `multipart` | Bounded streaming fields | All |
| `sse` | Pull-based `Sse` and validated `Event` | All |
| `cors` | Explicit origins, methods, headers, credentials and preflight | All |
| `compression` | Negotiated gzip for bounded buffered responses | All; streams pass through |
| `websocket` | Validated handshake, bounded frames/messages, tracked lifetime | HTTP/1.1 upgrade, including TLS |
| `tls` | TLS 1.3, ALPN, bounded session resumption cache | TCP |
| `http2` | HPACK, concurrent streams and flow control | TLS ALPN or explicit prior-knowledge cleartext |
| `http3` | QUIC/QPACK on separate UDP sockets; includes TLS | UDP; TCP can run on the same port |
| `opentelemetry` | Parent propagation, server spans, duration/active metrics | All |
| `ebpf` | Attach an already loaded reuseport BPF program | Linux TCP/UDP |

HTTP/3 uses a focused patch to the released `h3` decoder for strict malformed-field rejection. This workspace applies it automatically. An external application enabling `http3` must also add `[patch.crates-io] h3 = { path = "path/to/maximus/vendor/h3" }` to its own manifest. See [patch provenance and removal criteria](vendor/h3/PATCHES.md); Cargo does not inherit a dependency's patches.

The [feature example](examples/features.rs) runs JSON, uploads, SSE, gzip, CORS, WebSockets and typed routes. Set `MAXIMUS_CERT` and `MAXIMUS_KEY` to PEM files to enable TLS ALPN and HTTP/3 on the same numeric port. HTTP/3 is also available independently through `maximus-http3::endpoint/serve`. WebSockets use HTTP/1.1; extended CONNECT is not implemented. HTTP/2 cleartext uses prior knowledge, not an HTTP/1 Upgrade handshake.

```sh
cargo run --release -p maximus --all-features --example features
cargo doc --workspace --all-features --no-deps --open
```

## API and resource policy

See [API and operating guide](docs/guide.md) for extraction, state, middleware, streaming, limits and shutdown. See [qualification](docs/qualification.md) for commands and the [native implementation record](docs/native-http-qualification.md) for current evidence, and [benchmarks](benches/README.md) for reproducible k6/wrk comparisons, including [HTTP/2 receive-work measurements](benches/results/h2-receive.md), the [HTTP/2 hot-path experiments](benches/results/native-hot-paths.md) and [native engine comparison](benches/results/native-http-comparison.md). Reports retain their exact baseline revisions, failed load targets and measurement limits. Performance leadership is an objective, not a claim: results must distinguish load-generator limits, server saturation and VM noise.

The workspace separates runtime, transport, protocols, routing and framework integrations into twelve crates. Benchmark servers and fuzz tooling are separate workspaces. Optional integrations do not enter a minimal build. Unsafe product code is confined to the optional Linux eBPF syscall adapter, with its invariant documented at the call.

Licensed under MIT OR Apache-2.0. The API is version 0.2; see the [native HTTP migration](docs/native-http-migration.md) for changes to low-level transport and body integration. Protocol and lifecycle behavior is covered by executable tests, and API changes must update examples and compatibility tests.
