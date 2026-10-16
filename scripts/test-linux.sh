#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
docker run --rm --security-opt seccomp=unconfined \
  -v "$PWD:/work" -v maximus-cargo:/usr/local/cargo -v maximus-target:/work/target \
  -w /work rust:1.96-bookworm cargo test --workspace --all-features "$@"
