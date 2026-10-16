#!/usr/bin/env python3
"""Isolated Linux k6 comparisons. Raw results stay in artifacts/bench; no product deps."""
import argparse, csv, json, os, pathlib, subprocess, time
ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / 'artifacts/bench'
K6 = 'grafana/k6:1.6.0'
RUST = 'rust:1.96-bookworm'

def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)

def capture(*args):
    return subprocess.check_output(args, text=True).strip()

def counters(name):
    # Container CPU excludes load generation. VmRSS excludes the filesystem page cache.
    text = capture('docker', 'exec', name, 'sh', '-c', 'cat /sys/fs/cgroup/cpu.stat /proc/1/status; for p in $(cat /proc/1/task/1/children); do cat /proc/$p/status; done')
    fields = {}
    for line in text.splitlines():
        words = line.replace(':', '').split()
        if len(words) >= 2 and words[0] in ('usage_usec', 'VmRSS', 'Threads'):
            fields[words[0]] = fields.get(words[0], 0) + int(words[1])
    return fields

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engines', default='maximus,epoll,hyper,axum,actix')
    parser.add_argument('--paths', default='/plaintext,/json,/echo,/users/42')
    parser.add_argument('--rates', default='0', help='comma-separated rates; 0 is closed-loop saturation')
    parser.add_argument('--workers', type=int, default=1)
    parser.add_argument('--duration', default='30s')
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--vus', type=int, default=64)
    parser.add_argument('--label', default='run')
    parser.add_argument('--protocol', choices=['plain', 'h1', 'h2'], default='plain')
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    environment = {'date_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), 'args': vars(args),
        'docker': capture('docker', 'version', '--format', '{{.Server.Version}}'),
        'kernel': capture('docker', 'run', '--rm', RUST, 'uname', '-a'),
        'cpus': capture('docker', 'info', '--format', '{{.NCPU}}'),
        'images': {image: capture('docker', 'image', 'inspect', image, '--format', '{{json .RepoDigests}}') for image in (K6, RUST)}}
    (OUT / f'{args.label}-environment.json').write_text(json.dumps(environment, indent=2))
    if args.protocol != 'plain':
        run('openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', str(OUT/'key.pem'), '-out', str(OUT/'cert.pem'), '-days', '1', '-subj', '/CN=localhost', '-addext', 'subjectAltName=DNS:localhost,IP:127.0.0.1', stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    rows = []
    for repeat in range(args.repeats):
      for path in args.paths.split(','):
       for rate in args.rates.split(','):
        # Rotate order to reduce systematic thermal/time ordering bias.
        engines = args.engines.split(','); engines = engines[repeat % len(engines):] + engines[:repeat % len(engines)]
        for engine in engines:
            if engine == 'nginx' and path == '/echo':
                continue
            name = f'maximus-bench-{os.getpid()}-{len(rows)}'
            key = f'{args.label}-{engine}-{path.strip("/").replace("/", "-")}-w{args.workers}-r{rate}-n{repeat}'
            command = ['docker', 'run', '--rm', '-d', '--name', name, '--network', 'host', '--cpuset-cpus', ','.join(map(str, range(args.workers))), '--ulimit', 'nofile=65536:65536', '--security-opt', 'seccomp=unconfined']
            if args.protocol != 'plain':
                command += ['-v', f'{OUT}:/cert:ro', '-e', 'MAXIMUS_CERT=/cert/cert.pem', '-e', 'MAXIMUS_KEY=/cert/key.pem', '-e', f'PROTOCOL={args.protocol}']
            if engine == 'nginx':
                config = OUT / f'nginx-{args.workers}.conf'
                config.write_text((ROOT / 'benches/nginx.conf').read_text().replace('worker_processes 1;', f'worker_processes {args.workers};'))
                command += ['-v', f'{config}:/etc/nginx/nginx.conf:ro', 'nginx:1.29.6-alpine']
            else:
                command += ['-e', f'WORKERS={args.workers}', '-v', 'maximus-bench-target:/target:ro', RUST, '/target/release/maximus-comparison', engine]
            run(*command, stdout=subprocess.DEVNULL)
            try:
                for _ in range(100):
                    log = subprocess.check_output(['docker', 'logs', name], text=True, stderr=subprocess.STDOUT)
                    if 'ready' in log or engine == 'nginx': break
                    time.sleep(.1)
                base = ['docker', 'run', '--rm', '--network', 'host', '--cpuset-cpus', '2', '-u', '0', '-v', f'{ROOT / "benches/load.js"}:/load.js:ro', '-v', f'{OUT}:/results', '-e', f'PATHNAME={path}', '-e', f'VUS={args.vus}']
                if args.protocol != 'plain':
                    base += ['-v', f'{OUT}/cert.pem:/cert.pem:ro', '-e', 'SSL_CERT_FILE=/cert.pem', '-e', 'URL=https://localhost:8080', '-e', f'PROTOCOL={args.protocol}']
                with (OUT / f'{key}.log').open('w') as log:
                    run(*base, '-e', 'DURATION=3s', '-e', 'RESULT=/results/warmup.json', K6, 'run', '--quiet', '/load.js', stdout=log, stderr=subprocess.STDOUT)
                    before = counters(name)
                    test = base + ['-e', f'DURATION={args.duration}', '-e', f'RESULT=/results/{key}.json']
                    if rate != '0': test += ['-e', f'RATE={rate}']
                    result = subprocess.run(test + [K6, 'run', '--quiet', '/load.js'], stdout=log, stderr=subprocess.STDOUT)
                    after = counters(name)
                data = json.loads((OUT / f'{key}.json').read_text())['metrics']
                metric = lambda name, field, default=0: data.get(name, {}).get('values', {}).get(field, default)
                count = metric('http_reqs', 'count')
                row = dict(engine=engine, path=path, workers=args.workers, protocol=args.protocol, offered_rps=int(rate), repeat=repeat,
                    rps=metric('http_reqs', 'rate'), p50_ms=metric('http_req_duration', 'med'), p99_ms=metric('http_req_duration', 'p(99)'), p999_ms=metric('http_req_duration', 'p(99.9)'),
                    failures=metric('http_req_failed', 'passes'), checks_failed=metric('checks', 'fails'), dropped=metric('dropped_iterations', 'count'),
                    cpu_us_per_request=(after['usage_usec'] - before['usage_usec']) / max(count, 1), rss_kib=after['VmRSS'], exit=result.returncode)
                rows.append(row)
                with (OUT / f'{args.label}.csv').open('w') as file:
                    writer = csv.DictWriter(file, fieldnames=list(row)); writer.writeheader(); writer.writerows(rows)
                print(json.dumps(row), flush=True)
            finally:
                subprocess.run(['docker', 'rm', '-f', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

if __name__ == '__main__': main()
