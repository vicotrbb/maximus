#!/usr/bin/env python3
"""Run inside the Linux load-client container. Keep 20,000 validated connections live."""
import asyncio, json, os, time
COUNT = int(os.environ.get('CONNECTIONS', '20000'))
REQUEST = b'GET /plaintext HTTP/1.1\r\nHost: localhost\r\n\r\n'
async def response(reader):
    header = await asyncio.wait_for(reader.readuntil(b'\r\n\r\n'), 10)
    assert header.startswith(b'HTTP/1.1 200')
    fields = dict(line.lower().split(b': ', 1) for line in header.split(b'\r\n')[1:-2])
    body = await reader.readexactly(int(fields[b'content-length']))
    assert body == b'Hello, World!'
async def main():
    started = time.monotonic()
    connections = []
    semaphore = asyncio.Semaphore(128)
    async def open_one():
        async with semaphore:
            reader, writer = await asyncio.open_connection('127.0.0.1', 8080)
            writer.write(REQUEST); await writer.drain(); await response(reader)
            connections.append((reader, writer))
    await asyncio.gather(*(open_one() for _ in range(COUNT)))
    print(json.dumps({'phase':'connected', 'connections':len(connections), 'seconds':time.monotonic()-started}), flush=True)
    await asyncio.sleep(5)
    # Every connection must remain usable after the idle period.
    async def revisit(pair):
        async with semaphore:
            reader, writer = pair
            writer.write(REQUEST); await writer.drain(); await response(reader)
    await asyncio.gather(*(revisit(pair) for pair in connections))
    print(json.dumps({'phase':'revalidated', 'connections':len(connections)}), flush=True)
    await asyncio.sleep(5)
    for _, writer in connections: writer.close()
    await asyncio.gather(*(writer.wait_closed() for _, writer in connections))
    print(json.dumps({'phase':'closed', 'seconds':time.monotonic()-started}), flush=True)
asyncio.run(main())
