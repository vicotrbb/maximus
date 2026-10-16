#!/bin/sh
set -eu

# These crates need an executor, not networking or protocol adapters.
for package in maximus-core maximus-io maximus-runtime; do
    cargo check -p "$package" --no-default-features --lib
    dependencies=$(cargo tree -p "$package" --no-default-features --edges normal --prefix none --format '{p}')
    for dependency in $dependencies; do
        case "$dependency" in
            compio-io|compio-net|compio-tls|compio-quic)
                echo "unexpected $package I/O dependency: $dependency" >&2
                exit 1
                ;;
        esac
    done
done

cargo check -p maximus --no-default-features
for feature in macros dynamic json multipart sse cors ebpf opentelemetry websocket compression tls http2 http3; do
    cargo check -p maximus --no-default-features --features "$feature"
done
cargo check -p maximus --no-default-features --features tls,http2
cargo check -p maximus --all-features

# Independent clients may use these libraries; product protocol engines may not.
dependencies=$(cargo tree -p maximus --all-features --edges normal --prefix none --format '{p}')
for dependency in $dependencies; do
    case "$dependency" in
        hyper|hyper-util|tokio-util|h2)
            echo "unexpected product HTTP dependency: $dependency" >&2
            exit 1
            ;;
    esac
done
