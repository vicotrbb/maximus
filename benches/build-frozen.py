#!/usr/bin/env python3
"""Build a frozen source tree, invalidating local-package cache before every variant."""

import argparse
import fcntl
import hashlib
import json
import pathlib
import subprocess


def sources(root):
    files = [
        *root.glob("crates/**/src/**/*.rs"),
        *root.glob("crates/**/Cargo.toml"),
        *root.glob("vendor/**/*.rs"),
        *root.glob("vendor/**/Cargo.toml"),
        *root.glob("benches/baselines/src/**/*.rs"),
    ]
    files += [
        root / p
        for p in (
            "Cargo.toml",
            "Cargo.lock",
            "benches/baselines/Cargo.toml",
            "benches/baselines/Cargo.lock",
        )
    ]
    return {
        str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(set(files))
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=pathlib.Path, required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    parser.add_argument(
        "--bins",
        nargs="+",
        default=[
            "maximus-comparison",
            "events",
            "events-allocations",
            "status",
            "bodies",
            "allocations",
            "negotiation",
            "gzip-server",
        ],
    )
    args = parser.parse_args()
    if not args.bins or any(
        not name or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in name)
        for name in args.bins
    ):
        parser.error("binary names must be lowercase letters, digits, or hyphens")
    if not args.name or any(
        c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in args.name
    ):
        parser.error("name must be lowercase letters, digits, or hyphens")
    root = args.source.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    prefix = args.out / args.name
    if any(
        prefix.with_suffix(suffix).exists()
        for suffix in (".json", ".log", ".footprint")
    ):
        parser.error("refusing to overwrite an existing build manifest")
    lock_path = (
        pathlib.Path(__file__).resolve().parents[1] / "artifacts/frozen-build.lock"
    )
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    lock = lock_path.open("a")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        raise SystemExit("another frozen build owns the shared target volume")
    before = sources(root)
    base = [
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
        "maximus-bench-target:/work/benches/baselines/target",
        "-w",
        "/work/benches/baselines",
        "rust:1.96-bookworm",
    ]
    # Separate snapshots use the same /work path. Cargo's timestamp check cannot
    # distinguish an older restored file from one that generated a cached object.
    # Clean every local package, then mark all local sources using the guest clock.
    shell = """set -eu
cargo clean --release -p maximus-core -p maximus-http1 -p maximus-http2 -p maximus-router -p maximus-runtime -p maximus-net -p maximus-io -p maximus-tls -p maximus-macros -p maximus -p maximus-comparison
find /work/crates /work/benches/baselines/src -name '*.rs' -exec touch {} +
"""
    shell = (
        shell
        + "cargo build --release --locked "
        + " ".join("--bin " + name for name in args.bins)
        + "\n"
    )
    with prefix.with_suffix(".log").open("w") as log:
        subprocess.run(
            base + ["sh", "-c", shell], stdout=log, stderr=subprocess.STDOUT, check=True
        )
    after = sources(root)
    if before != after:
        raise SystemExit("source content changed during build")
    destination = f"/work/benches/baselines/target/2026-10-21/{args.name}"
    copy = f"""set -eu
mkdir -p {destination}
for binary in {" ".join(args.bins)}; do
cp target/release/$binary {destination}/$binary
done
sha256sum {destination}/*
size {destination}/*
"""
    result = subprocess.check_output(base + ["sh", "-c", copy], text=True)
    prefix.with_suffix(".footprint").write_text(result)
    prefix.with_suffix(".json").write_text(
        json.dumps(
            {
                "sources": before,
                "command": base,
                "build_script": shell,
                "copy_script": copy,
                "image_id": subprocess.check_output(
                    [
                        "docker",
                        "image",
                        "inspect",
                        "rust:1.96-bookworm",
                        "--format",
                        "{{.Id}}",
                    ],
                    text=True,
                ).strip(),
                "footprint": result,
            },
            indent=2,
        )
        + "\n"
    )
    print(args.name, result, flush=True)


if __name__ == "__main__":
    main()
