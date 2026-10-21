#!/usr/bin/env python3
"""Export the current source and verify three isolated, unqualified candidates."""
import argparse
import gzip
import hashlib
import io
import json
import pathlib
import subprocess
import tarfile
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
VARIANTS = {
    'borrowed-errors': 'crates/core/src/lib.rs',
    'h1-empty-tracking': 'crates/http1/src/lib.rs',
    'h2-borrowed-timer': 'crates/http2/src/lib.rs',
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=pathlib.Path, help='new bundle directory; existing paths are refused')
    args = parser.parse_args()
    original_head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    listed = subprocess.check_output(['git', 'ls-files', '-z'], cwd=ROOT).decode().split('\0')
    listed += ['benches/baselines/src/bin/components.rs', 'benches/compare-ownership.py', 'benches/test_run.py', 'benches/export-qualification.py']
    listed += [f'benches/qualification-candidates/{name}.patch' for name in VARIANTS]
    listed += ['benches/qualification-candidates/README.md', 'benches/qualification-candidates/original-http1.rs', 'benches/qualification-candidates/original-reference.json']
    prefixes = ('crates/', 'vendor/', 'scripts/', 'tests/', 'examples/', 'fuzz/', '.cargo/', 'docs/')
    bench_files = {'benches/README.md', 'benches/HARDWARE.md', 'benches/Dockerfile.tools', 'benches/run.py',
                   'benches/compare-ownership.py', 'benches/load.js', 'benches/wrk.lua',
                   'benches/nginx.conf', 'benches/profile.sh', 'benches/test_run.py',
                   'benches/export-qualification.py'}
    files = {}
    modes = {}
    for name in sorted(set(listed) - {''}):
        relative = pathlib.PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts:
            raise SystemExit(f'unsafe source path: {name}')
        include = '/' not in name or name.startswith(prefixes) or name.startswith(('benches/baselines/', 'benches/qualification-candidates/')) or name in bench_files
        if not include:
            continue
        path = ROOT / name
        if path.is_symlink() or not path.is_file():
            raise SystemExit(f'expected regular source file: {name}')
        files[name] = path.read_bytes()
        modes[name] = 0o755 if path.stat().st_mode & 0o111 else 0o644
    reference = json.loads(files['benches/qualification-candidates/original-reference.json'])
    if sha(files[reference['source']]) != reference['sha256']:
        raise SystemExit('original reference hash mismatch')
    candidate_hashes = {}
    with tempfile.TemporaryDirectory(prefix='maximus-qualification-') as directory:
        source = pathlib.Path(directory)
        for name, data in files.items():
            path = source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        for variant, target in VARIANTS.items():
            patch = f'benches/qualification-candidates/{variant}.patch'
            subprocess.run(['git', 'apply', '--check', str(source / patch)], cwd=source, check=True)
            subprocess.run(['git', 'apply', str(source / patch)], cwd=source, check=True)
            actual = {str(path.relative_to(source)) for path in source.rglob('*') if path.is_file() or path.is_symlink()}
            if actual != set(files) or any(path.is_symlink() for path in source.rglob('*')):
                raise SystemExit(f'{variant}: unexpected source paths after patch')
            changed = {name: sha((source / name).read_bytes()) for name, data in files.items() if (source / name).read_bytes() != data}
            if set(changed) != {target}:
                raise SystemExit(f'{variant}: unexpected changed files: {sorted(changed)}')
            if 'connect && !switching' not in (source / 'crates/http1/src/lib.rs').read_text():
                raise SystemExit(f'{variant}: corrected CONNECT predicate is missing')
            candidate_hashes[variant] = changed
            (source / target).write_bytes(files[target])
    # Create output only after all patches have been checked against the captured bytes.
    args.destination.mkdir(parents=True, exist_ok=False)
    archive = args.destination / 'source.tar.gz'
    with archive.open('wb') as raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed, tarfile.open(fileobj=compressed, mode='w') as tar:
        for name, data in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = modes[name]
            tar.addfile(info, io.BytesIO(data))
    manifest = {
        'scope': 'Source preparation only. No candidate is accepted, benchmarked, or qualified by this export.',
        'original_head': original_head,
        'archive_sha256': sha(archive.read_bytes()),
        'files_sha256': {name: sha(data) for name, data in files.items()},
        'variants': candidate_hashes,
        'original_performance_reference': reference,
        'verification': 'Each patch applied in a separate restored source state; exactly its named library changed; corrected CONNECT predicate retained.',
    }
    (args.destination / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps({'destination': str(args.destination.resolve()), 'files': len(files), 'archive_bytes': archive.stat().st_size, 'archive_sha256': manifest['archive_sha256'], 'variants': list(candidate_hashes)}, indent=2))


if __name__ == '__main__':
    main()
