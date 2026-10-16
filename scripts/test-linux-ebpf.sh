#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
docker run --rm --security-opt seccomp=unconfined --cap-add BPF --cap-add NET_ADMIN \
  -v "$PWD:/work" -v maximus-cargo:/usr/local/cargo -v maximus-target:/work/target \
  -w /work rust:1.96-bookworm cargo test -p maximus-net --features ebpf --test ebpf -- --include-ignored
