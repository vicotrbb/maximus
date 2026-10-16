#!/usr/bin/env python3
"""Run inside the Linux load-client container. Keep 20,000 validated connections live."""
import asyncio, json, os, pathlib, subprocess, sys, time
COUNT = int(os.environ.get('CONNECTIONS', '20000'))
REQUEST = b'GET /plaintext HTTP/1.1\r\nHost: localhost\r\n\r\n'
async def response(reader):
    header = await asyncio.wait_for(reader.readuntil(b'\r\n\r\n'), 10)
    assert header.startswith(b'HTTP/1.1 200')
    fields = dict(line.lower().split(b': ', 1) for line in header.split(b'\r\n')[1:-2])
    body = await reader.readexactly(int(fields[b'content-length']))
    assert body == b'Hello, World!'
async def cycle(index):
    started = time.monotonic()
    connections = []
    semaphore = asyncio.Semaphore(128)
    async def open_one():
        async with semaphore:
            reader, writer = await asyncio.open_connection('127.0.0.1', 8080)
            writer.write(REQUEST); await writer.drain(); await response(reader)
            connections.append((reader, writer))
    await asyncio.gather(*(open_one() for _ in range(COUNT)))
    print(json.dumps({'cycle':index, 'phase':'connected', 'connections':len(connections), 'seconds':time.monotonic()-started}), flush=True)
    await asyncio.sleep(5)
    # Every connection must remain usable after the idle period.
    async def revisit(pair):
        async with semaphore:
            reader, writer = pair
            writer.write(REQUEST); await writer.drain(); await response(reader)
    await asyncio.gather(*(revisit(pair) for pair in connections))
    print(json.dumps({'cycle':index, 'phase':'revalidated', 'connections':len(connections)}), flush=True)
    await asyncio.sleep(5)
    for _, writer in connections: writer.close()
    await asyncio.gather(*(writer.wait_closed() for _, writer in connections))
    await asyncio.sleep(1)
    print(json.dumps({'cycle':index, 'phase':'closed', 'seconds':time.monotonic()-started}), flush=True)
async def main():
    for index in range(int(os.environ.get('CYCLES', '3'))):
        await cycle(index)

def docker():
    from run import ROOT, RUST, capture, counters
    name = f'maximus-connections-{os.getpid()}'
    def sample(event):
        event.update(counters(name))
        event['fds'] = int(capture('docker', 'exec', name, 'sh', '-c', 'ls -U /proc/1/fd | wc -l'))
        print(json.dumps(event), flush=True)
    subprocess.run(['docker', 'run', '--rm', '-d', '--name', name, '--network', 'host', '--cpuset-cpus', '0', '--ulimit', 'nofile=65536:65536', '--security-opt', 'seccomp=unconfined', '-e', 'IDLE_CONNECTIONS=25000', '-v', 'maximus-bench-target:/target:ro', RUST, '/target/release/maximus-comparison', 'maximus'], check=True, stdout=subprocess.DEVNULL)
    try:
        for _ in range(100):
            if 'ready' in subprocess.check_output(['docker', 'logs', name], text=True, stderr=subprocess.STDOUT): break
            time.sleep(.1)
        sample({'phase':'baseline'})
        with subprocess.Popen(['docker', 'run', '--rm', '--network', 'host', '--cpuset-cpus', '2', '--ulimit', 'nofile=65536:65536', '-e', f'CONNECTIONS={COUNT}', '-e', f'CYCLES={os.environ.get("CYCLES", "3")}', '-v', f'{ROOT}:/work:ro', 'maximus-tools', 'python3', '/work/benches/connections.py'], stdout=subprocess.PIPE, text=True) as client:
            for line in client.stdout: sample(json.loads(line))
            if client.wait() != 0: raise RuntimeError('connection client failed')
        sample({'phase':'finished'})
    finally:
        subprocess.run(['docker', 'rm', '-f', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

if __name__ == '__main__':
    if '--docker' in sys.argv: docker()
    else: asyncio.run(main())
