#!/usr/bin/env python3
"""Observe native RSS after checked large uploads on connections held open."""
import argparse
import hashlib
import http.client
import json
import os
import pathlib
import platform
import resource
import socket
import subprocess
import time


def trial(binary, engine, count, prefix):
    body = b'x' * 131072
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    env = dict(os.environ, ADDRESS=f'127.0.0.1:{port}', WORKERS='1', PROTOCOL='plain')
    connections = []
    observations = []
    with prefix.with_suffix('.log').open('w') as log:
        server = subprocess.Popen([str(binary.resolve()), 'epoll' if engine == 'maximus' else engine],
                                  env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(200):
                if server.poll() is not None:
                    raise ValueError('server exited before listening')
                try:
                    with socket.create_connection(('127.0.0.1', port), timeout=.05):
                        break
                except OSError:
                    time.sleep(.05)
            else:
                raise ValueError('server startup timed out')
            def rss():
                return int(subprocess.check_output(['ps', '-o', 'rss=', '-p', str(server.pid)], text=True).strip())
            initial = rss()
            for _ in range(count):
                connection = http.client.HTTPConnection('127.0.0.1', port, timeout=10)
                connections.append(connection)
                connection.request('POST', '/echo', body=body)
                response = connection.getresponse()
                if response.status != 200 or response.read() != body or response.will_close:
                    raise ValueError('large echo did not preserve the connection and exact payload')
                transport = connection.sock
                connection.request('GET', '/plaintext')
                response = connection.getresponse()
                if response.status != 200 or response.read() != b'Hello, World!' or response.will_close or connection.sock is not transport:
                    raise ValueError('following request failed or used a new connection')
            for _ in range(3):
                time.sleep(.5)
                observations.append(rss())
            # Check every held connection again: a lower RSS caused by silently
            # closing connections must not be mistaken for improved retention.
            for connection in connections:
                transport = connection.sock
                connection.request('GET', '/plaintext')
                response = connection.getresponse()
                if response.status != 200 or response.read() != b'Hello, World!' or response.will_close or connection.sock is not transport:
                    raise ValueError('held connection was lost')
            if server.poll() is not None:
                raise ValueError('server exited during held-connection checks')
            return dict(engine=engine, connections=count, upload_bytes=len(body), initial_rss_kib=initial,
                        held_rss_kib=observations, validated_requests=count * 3,
                        server_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), passed=True)
        finally:
            for connection in connections:
                connection.close()
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('baseline', 'candidate', 'out'):
        parser.add_argument('--' + name, required=True, type=pathlib.Path)
    parser.add_argument('--connections', type=int, default=256)
    parser.add_argument('--repeats', type=int, default=3)
    args = parser.parse_args()
    if min(args.connections, args.repeats) < 1:
        parser.error('connections and repeats must be positive')
    args.out.mkdir(parents=True, exist_ok=False)
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    if soft < args.connections + 128:
        resource.setrlimit(resource.RLIMIT_NOFILE, (min(hard, args.connections + 128), hard))
    rows = []
    for repeat in range(args.repeats):
        order = [('baseline', args.baseline, 'maximus'), ('candidate', args.candidate, 'maximus')]
        if repeat % 2:
            order.reverse()
        peers = ['hyper', 'axum']
        if repeat % 2:
            peers.reverse()
        order += [('peer', args.candidate, engine) for engine in peers]
        for variant, binary, engine in order:
            row = dict(repeat=repeat, variant=variant, engine=engine)
            try:
                row.update(trial(binary, engine, args.connections, args.out / f'{repeat}-{variant}-{engine}'))
            except (OSError, ValueError, http.client.HTTPException, subprocess.SubprocessError) as error:
                row.update(passed=False, error=str(error))
            rows.append(row)
            (args.out / 'results.json').write_text(json.dumps(dict(platform=platform.platform(), scope='native Poll/plain HTTP1 post-upload RSS; not live requested heap', rows=rows), indent=2) + '\n')
            print(repeat, variant, engine, 'PASS' if row['passed'] else row['error'], flush=True)
            if not row['passed']:
                raise SystemExit(1)


if __name__ == '__main__':
    main()
