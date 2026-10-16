#!/bin/sh
# Verify Cargo's non-propagating patch behavior using the documented external consumer.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/src"
cat > "$tmp/Cargo.toml" <<EOF
[package]
name = "maximus-downstream-check"
version = "0.0.0"
edition = "2024"
[dependencies]
maximus = { path = "$root/crates/framework", features = ["http3", "http2", "tls"] }
[patch.crates-io]
h3 = { path = "$root/vendor/h3" }
EOF
cat > "$tmp/src/main.rs" <<'EOF'
fn main() {
    let _server = maximus::Server::new("127.0.0.1:0".parse().unwrap());
    let _limits = maximus::Limits::default();
}
EOF
CARGO_TARGET_DIR="$root/target/downstream" cargo check --manifest-path "$tmp/Cargo.toml"
resolved=$(cargo tree --manifest-path "$tmp/Cargo.toml" -i h3 --prefix none | head -1)
[ "$resolved" = "h3 v0.0.8 ($root/vendor/h3)" ] || { echo "unexpected h3 source: $resolved" >&2; exit 1; }
printf '%s\n' "$resolved"
