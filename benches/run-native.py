#!/usr/bin/env python3
"""Checked native socket comparisons. Native results are never pooled with Linux."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import resource
import socket
import subprocess
import time

from workloads import DEFAULT_CASES, SOCKET_CASES

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('socket_runner', ROOT / 'benches/run.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def cpu():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_trial(options, binary, engine, protocol, path, size, prefix, cert):
    prefix.parent.mkdir(parents=True, exist_ok=True)
    config = prefix.with_suffix('.config.json')
    config.write_text('{}\n')
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    address = f'127.0.0.1:{port}'
    env = {key: value for key, value in os.environ.items() if not key.startswith('K6_')}
    env.update(ADDRESS=address, WORKERS=str(options.workers), PROTOCOL=protocol)
    if cert:
        env.update(MAXIMUS_CERT=str(cert[0]), MAXIMUS_KEY=str(cert[1]), SSL_CERT_FILE=str(cert[0]))
    argument = 'epoll' if engine == 'maximus' and options.backend == 'poll' and binary.name != 'gzip-server' else engine
    command = [str(binary.resolve()), argument]
    with prefix.with_suffix('.server.log').open('w') as log:
        server = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
        started = time.monotonic()
        measurements = []
        rss = []
        client_cpu = 0
        row = None
        try:
            for _ in range(200):
                if server.poll() is not None:
                    raise ValueError(f'server exited: {command}; see {log.name}')
                try:
                    with socket.create_connection(('127.0.0.1', port), timeout=.05):
                        break
                except OSError:
                    time.sleep(.05)
            else:
                raise ValueError('server did not listen within 10 seconds')
            for phase, duration in (('warmup', '3s'), ('measured', options.duration)):
                result = prefix.with_name(prefix.name + '-' + phase).with_suffix('.json')
                if result.exists():
                    raise ValueError(f'refusing to overwrite {result}')
                client_env = dict(env, K6_NO_USAGE_REPORT='true', GOMAXPROCS='4',
                                  GODEBUG='x509sslcertoverrideplatform=1',
                                  URL=('http://' if protocol == 'plain' else 'https://') + address,
                                  PATHNAME=path, BODY_BYTES=str(size), VUS=str(options.vus),
                                  CHURN='1' if getattr(options, 'churn', False) else '0',
                                  DURATION=duration, RESULT=str(result.resolve()))
                if options.rate:
                    client_env['RATE'] = str(options.rate)
                else:
                    client_env.pop('RATE', None)
                before = cpu()
                with result.with_suffix('.log').open('w') as client_log:
                    completed = subprocess.run([str(options.k6.resolve()), 'run', '--quiet', '--no-color', '--config', str(config.resolve()),
                                                str(ROOT / 'benches/load.js')], env=client_env,
                                               stdout=client_log, stderr=subprocess.STDOUT,
                                               timeout=180)
                client_cpu += cpu() - before
                summary = json.loads(result.read_text())
                counts = runner.validate_k6_iterations(summary)
                def metric(name, key):
                    return summary.get('metrics', {}).get(name, {}).get('values', {}).get(key, 0)
                failures = dict(failures=metric('http_req_failed', 'passes'),
                                checks_failed=metric('checks', 'fails'),
                                network_errors=metric('network_errors', 'count'),
                                dropped=metric('dropped_iterations', 'count'), exit=completed.returncode)
                measurements.append(counts[1])
                if phase == 'warmup' and any(failures.values()):
                    raise ValueError(f'failed warmup: {failures}; raw {result}')
                observed = subprocess.check_output(['ps', '-o', 'rss=', '-p', str(server.pid)], text=True)
                rss.append(int(observed.strip()))
                if phase == 'measured':
                    row = dict(engine=engine, protocol=protocol, path=path,
                               body_bytes=size if path == '/echo' else 0, generator='k6',
                               requests=counts[1], iterations_started=counts[0], iterations_completed=counts[1],
                               rps=metric('http_reqs', 'rate'), p50_ms=metric('http_req_duration', 'med'),
                               p99_ms=metric('http_req_duration', 'p(99)'), p999_ms=metric('http_req_duration', 'p(99.9)'),
                               **failures)
            if server.poll() is not None:
                raise ValueError('server exited during measurement')
        finally:
            # Only the server is reaped in this interval. Client and ps children
            # were already waited for; their CPU is deliberately excluded.
            before_stop = cpu()
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
            server_cpu = cpu() - before_stop
            lifetime = time.monotonic() - started
        row.update(cpu_us_per_request=server_cpu * 1e6 / sum(measurements),
                   server_cpu_percent=100 * server_cpu / lifetime,
                   client_cpu_percent=100 * client_cpu / lifetime,
                   rss_kib=rss[-1], peak_rss_kib=max(rss),
                   cpu_request_count=sum(measurements), cpu_interval='server lifetime including warmup',
                   server_lifetime_seconds=lifetime, rss_scope='two post-client observations, not live heap or continuous peak',
                   command=command)
        return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('baseline', 'candidate', 'k6', 'out'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True)
    parser.add_argument('--cases', default=DEFAULT_CASES)
    parser.add_argument('--repeats', type=int, default=7)
    parser.add_argument('--duration', default='10s')
    parser.add_argument('--vus', type=int, default=64)
    parser.add_argument('--workers', type=int, default=1)
    parser.add_argument('--rate', type=int, default=0)
    parser.add_argument('--backend', choices=('poll', 'io-uring'), default='poll')
    parser.add_argument('--no-peers', action='store_true')
    parser.add_argument('--churn', action='store_true', help='disable HTTP connection reuse consistently')
    args = parser.parse_args()
    cases = [int(case) for case in args.cases.split(',')]
    if len(cases) != len(set(cases)) or any(case not in range(len(SOCKET_CASES)) for case in cases) or min(args.repeats, args.vus, args.workers) < 1:
        parser.error('cases must be unique valid indices; repeats, vus and workers must be positive')
    args.out.mkdir(parents=True, exist_ok=False)
    metadata = dict(options={key: str(value) if isinstance(value, pathlib.Path) else value for key, value in vars(args).items()},
                    platform=platform.platform(), machine=platform.machine(), cpu_affinity='uncontrolled',
                    client_gomaxprocs=4, tls_verification=True, controller_sha256=digest(pathlib.Path(__file__).resolve()),
                    workload_sha256=digest(ROOT / 'benches/load.js'), k6_sha256=digest(args.k6),
                    binaries={variant: {name: digest(directory / name) for name in {SOCKET_CASES[case][4] for case in cases}}
                              for variant, directory in (('baseline', args.baseline), ('candidate', args.candidate))})
    (args.out / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    cert = None
    if any(SOCKET_CASES[case][0] != 'plain' for case in cases):
        cert = (args.out.resolve() / 'cert.pem', args.out.resolve() / 'key.pem')
        subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', str(cert[1]),
                        '-out', str(cert[0]), '-days', '2', '-subj', '/CN=localhost', '-addext',
                        'subjectAltName=DNS:localhost,IP:127.0.0.1'], check=True, capture_output=True)
        cert[1].chmod(0o600)
    trials = []
    for repeat in range(args.repeats):
        for case in cases:
            protocol, paths, size, _, binary = SOCKET_CASES[case]
            for variant in (('baseline', 'candidate') if repeat % 2 == 0 else ('candidate', 'baseline')):
                directory = args.baseline if variant == 'baseline' else args.candidate
                engines = ['maximus'] if variant == 'baseline' or args.no_peers else ['maximus', 'hyper', 'axum']
                if len(engines) > 1:
                    rotation = repeat % len(engines)
                    engines = engines[rotation:] + engines[:rotation]
                label = f'{repeat}-{case}-{variant}'
                trial = dict(repeat=repeat, case=case, variant=variant, label=label, engines=engines,
                             command=['run-native.py', '--backend', args.backend], rows=[], passed=False, exit=1)
                try:
                    for engine in engines:
                        for index, path in enumerate(paths.split(',')):
                            prefix = args.out / f'{label}-{engine}-{index}'
                            row = run_trial(args, directory / binary, engine, protocol, path, size, prefix,
                                            cert if protocol != 'plain' else None)
                            trial['rows'].append(row)
                    trial['passed'] = all(not any(row[key] for key in ('failures', 'checks_failed', 'network_errors', 'dropped', 'exit')) for row in trial['rows'])
                    trial['exit'] = 0 if trial['passed'] else 1
                except (OSError, ValueError, subprocess.SubprocessError) as error:
                    trial['error'] = str(error)
                trials.append(trial)
                (args.out / 'campaign.json').write_text(json.dumps(trials, indent=2) + '\n')
                print(label, 'PASS' if trial['passed'] else 'FAIL', trial.get('error', ''), flush=True)
                if not trial['passed']:
                    raise SystemExit(1)


if __name__ == '__main__':
    main()
