#!/bin/sh
set -eu
cargo check -p maximus --no-default-features
for feature in macros dynamic json multipart sse cors ebpf opentelemetry websocket compression tls http2 http3; do
    cargo check -p maximus --no-default-features --features "$feature"
done
cargo check -p maximus --no-default-features --features tls,http2
cargo check -p maximus --all-features
