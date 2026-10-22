#!/usr/bin/env python3
"""Build frozen native variants at one source path with local cache invalidation."""
import argparse
import fcntl
import importlib.util
import json
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('frozen', ROOT / 'benches/build-frozen.py')
frozen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(frozen)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=pathlib.Path, required=True)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    parser.add_argument('--target', type=pathlib.Path, required=True)
    args = parser.parse_args()
    args.out = args.out.resolve()
    args.target = args.target.resolve()
    args.out.mkdir(parents=True, exist_ok=False)
    workspace = ROOT / 'artifacts/native-build'
    workspace.mkdir(parents=True, exist_ok=True)
    with (workspace / 'build.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        canonical = workspace / 'source'
        # Only this builder owns this ignored staging directory.
        if canonical.exists():
            shutil.rmtree(canonical)
        before = frozen.sources(args.source.resolve())
        shutil.copytree(args.source, canonical, ignore=shutil.ignore_patterns('target', 'artifacts', '.git', '.superpowers'))
        for path in canonical.rglob('*.rs'):
            path.touch()
        import os
        env = dict(os.environ, CARGO_TARGET_DIR=str(args.target))
        packages = ['maximus-core', 'maximus-http1', 'maximus-http2', 'maximus-router', 'maximus-runtime', 'maximus-net', 'maximus-io', 'maximus-tls', 'maximus-macros', 'maximus', 'maximus-comparison']
        clean = ['cargo', 'clean', '--release'] + [item for package in packages for item in ('-p', package)]
        build = ['cargo', 'build', '--release', '--locked', '--bin', 'maximus-comparison', '--bin', 'gzip-server', '--bin', 'allocations']
        with (args.out / 'build.log').open('w') as log:
            for command in (clean, build):
                subprocess.run(command, cwd=canonical / 'benches/baselines', env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        if before != frozen.sources(args.source.resolve()) or before != frozen.sources(canonical):
            raise SystemExit('source changed during native build')
        for name in ('maximus-comparison', 'gzip-server', 'allocations'):
            shutil.copy2(args.target / 'release' / name, args.out / name)
        (args.out / 'manifest.json').write_text(json.dumps(dict(sources=before, clean=clean, build=build,
                source_path=str(canonical), target_path=str(args.target),
                rustc=subprocess.check_output(['rustc', '-Vv'], text=True)), indent=2) + '\n')
        print(args.out, flush=True)


if __name__ == '__main__':
    main()
