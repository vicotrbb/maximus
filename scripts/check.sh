#!/bin/sh
set -eu
cargo fmt --all --check
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
cargo test -p maximus --no-default-features
scripts/check-features.sh
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps
