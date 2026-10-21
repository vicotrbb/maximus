#!/usr/bin/env python3
"""Compare frozen ownership candidates, retaining every trial and failed target."""
import argparse
import csv
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--label', required=True)
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--duration', default='15s')
    parser.add_argument('--generator', choices=['k6', 'wrk'], default='k6')
    parser.add_argument('--protocol', choices=['any', 'plain', 'h2'], default='any')
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error('--repeats must be positive')
    if args.generator == 'wrk' and args.protocol == 'h2':
        parser.error('wrk supports plain HTTP/1 only')
    cases = [
        ('plain', '/plaintext', 1024, 'dynamic'),
        ('plain', '/users/42', 1024, 'dynamic'),
        ('plain', '/error', 1024, 'dynamic'),
        ('plain', '/echo', 1024, 'dynamic'),
        ('h2', '/error', 1024, 'maximus'),
        ('h2', '/echo', 131072, 'maximus'),
        ('plain', '/plaintext', 1024, 'maximus'),
    ]
    outcomes = []
    certificate_ready = False
    for repeat in range(args.repeats):
        variants = [('baseline', args.baseline), ('candidate', args.candidate)]
        if repeat % 2:
            variants.reverse()
        for case, (protocol, path, size, engine) in enumerate(cases):
            if args.protocol != 'any' and protocol != args.protocol:
                continue
            if args.generator == 'wrk' and protocol != 'plain':
                continue
            for variant, binary in variants:
                label = f'{args.label}-{repeat}-{case}-{variant}'
                output = ROOT / 'artifacts/bench' / f'{label}.csv'
                if list(output.parent.glob(f'{label}*')):
                    raise SystemExit(f'refusing to overwrite trial {label}')
                engines = [engine]
                if variant == 'candidate':
                    engines += ['hyper', 'axum']
                    offset = repeat % len(engines)
                    engines = engines[offset:] + engines[:offset]
                command = [sys.executable, str(ROOT / 'benches/run.py'),
                           '--binary', binary, '--engines', ','.join(engines),
                           '--label', label, '--paths', path, '--protocol', protocol,
                           '--body-bytes', str(size), '--client-cpus', '1,2',
                           '--duration', args.duration, '--repeats', '1', '--generator', args.generator]
                if protocol != 'plain' and certificate_ready:
                    command += ['--reuse-certificate']
                result = subprocess.run(command, cwd=ROOT, check=False)
                if protocol != 'plain' and result.returncode == 0:
                    certificate_ready = True
                rows = list(csv.DictReader(output.open())) if output.exists() else []
                passed = result.returncode == 0 and len(rows) == len(engines) and all(
                    float(row[key]) == 0
                    for row in rows for key in ('failures', 'checks_failed', 'network_errors', 'dropped', 'exit')
                )
                outcomes.append(dict(label=label, passed=passed, exit=result.returncode))
                destination = output.parent / f'{args.label}-outcomes.json'
                destination.write_text(json.dumps(outcomes, indent=2) + '\n')
    if not all(item['passed'] for item in outcomes):
        raise SystemExit('campaign contains failed targets; see retained outcomes and rows')


if __name__ == '__main__':
    main()
