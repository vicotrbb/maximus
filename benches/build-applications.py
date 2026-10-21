#!/usr/bin/env python3
import argparse
import fcntl
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(
    description="Build clean native application footprints from frozen source."
)
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--name", required=True)
parser.add_argument("--out", type=Path, required=True)
args = parser.parse_args()
if not args.name or any(
    c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in args.name
):
    parser.error("name must be lowercase letters, digits, or hyphens")
out = args.out.resolve()
source = args.source.resolve()
if (out / (args.name + "-application-source")).is_relative_to(source):
    parser.error("application clone must be outside the frozen source tree")
out.mkdir(parents=True, exist_ok=True)
lock_path = Path(__file__).resolve().parents[1] / "artifacts/frozen-build.lock"
lock_path.parent.mkdir(parents=True, exist_ok=True)
lock = lock_path.open("a")
fcntl.flock(lock, fcntl.LOCK_EX)
variant = args.name
root = out / (variant + "-application-source")
shutil.copytree(source, root)
fixture = root / "examples/sse-size.rs"
shutil.copyfile(str(Path(__file__).with_name("sse-size.rs")), fixture)
shutil.copyfile(
    str(Path(__file__).with_name("compression-size.rs")),
    root / "examples/compression-size.rs",
)
shutil.copyfile(
    Path(__file__).with_name("tls-sse-size.rs"), root / "examples/tls-sse-size.rs"
)
manifest = root / "crates/framework/Cargo.toml"
manifest.write_text(
    manifest.read_text()
    + """\n[[example]]
name = "sse-size"
path = "../../examples/sse-size.rs"
required-features = ["sse"]

[[example]]
name = "compression-size"
path = "../../examples/compression-size.rs"
required-features = ["compression"]

[[example]]
name = "tls-sse-size"
path = "../../examples/tls-sse-size.rs"
required-features = ["sse", "tls", "http2"]
"""
)


def source_hashes():
    files = [
        *root.glob("crates/**/*.rs"),
        *root.glob("vendor/**/*.rs"),
        *root.glob("examples/*.rs"),
        *root.glob("**/Cargo.toml"),
        *root.glob("**/Cargo.lock"),
    ]
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(set(files))
    }


before = source_hashes()
command = [
    "docker",
    "run",
    "--rm",
    "--security-opt",
    "seccomp=unconfined",
    "-v",
    f"{root}:/work",
    "-v",
    "maximus-cargo:/usr/local/cargo",
    "-v",
    "maximus-bench-target:/target",
    "-w",
    "/work",
    "rust:1.96-bookworm",
    "sh",
    "-c",
    f"""set -eu
export CARGO_TARGET_DIR=/target
cargo clean --release -p maximus-core -p maximus-http1 -p maximus-router -p maximus-runtime -p maximus-net -p maximus-io -p maximus-macros -p maximus
find /work/crates -name '*.rs' -exec touch {{}} +
cargo build --release --locked -p maximus --example hello
mkdir -p /target/2026-10-21/application-{variant}
cp /target/release/examples/hello /target/2026-10-21/application-{variant}/hello
cargo build --release --locked -p maximus --features sse --example sse-size
cp /target/release/examples/sse-size /target/2026-10-21/application-{variant}/sse
cargo build --release --locked -p maximus --features compression --example compression-size
cp /target/release/examples/compression-size /target/2026-10-21/application-{variant}/compression
cargo build --release --locked -p maximus --features sse,tls,http2 --example tls-sse-size
cp /target/release/examples/tls-sse-size /target/2026-10-21/application-{variant}/tls-sse
cargo build --release --locked -p maximus --all-features --example features
cp /target/release/examples/features /target/2026-10-21/application-{variant}/features
sha256sum /target/2026-10-21/application-{variant}/*
size /target/2026-10-21/application-{variant}/*
size -A /target/2026-10-21/application-{variant}/*
stat -c "%n %s" /target/2026-10-21/application-{variant}/*
""",
]
with (out / f"application-{variant}.log").open("w") as f:
    subprocess.run(command, stdout=f, stderr=subprocess.STDOUT, check=True)
if before != source_hashes():
    raise SystemExit("source content changed during build")
(out / f"application-{variant}.json").write_text(
    json.dumps(
        {
            "command": command,
            "sources": before,
            "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
            "framework_manifest_sha256": hashlib.sha256(
                manifest.read_bytes()
            ).hexdigest(),
        },
        indent=2,
    )
)
