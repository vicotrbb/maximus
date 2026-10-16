#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
docker build -f benches/Dockerfile.tools -t maximus-tools .
docker run --rm --security-opt seccomp=unconfined -v "$PWD:/work" -v maximus-cargo:/usr/local/cargo -v maximus-target:/work/target -w /work maximus-tools sh -c 'cargo build -p maximus --all-features --examples && python3 -m venv /tmp/interop && /tmp/interop/bin/pip install -r scripts/interop-requirements.txt && /tmp/interop/bin/python scripts/interop.py && python3 tests/wire.py'
