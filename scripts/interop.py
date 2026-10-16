#!/usr/bin/env python3
"""Independent clients against the features example, with verified local certificates."""
import asyncio, json, os, pathlib, ssl, subprocess
import httpx, websockets
from aioquic.asyncio import connect, QuicConnectionProtocol
from aioquic.h3.connection import H3Connection
from aioquic.h3.events import HeadersReceived, DataReceived
from aioquic.quic.configuration import QuicConfiguration
from aioquic.quic.events import StreamReset
ROOT = pathlib.Path(__file__).resolve().parents[1]
ART = ROOT / 'artifacts/interop'

class H3Client(QuicConnectionProtocol):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.http = H3Connection(self._quic)
        self.pending = {}
    def quic_event_received(self, event):
        if isinstance(event, StreamReset) and event.stream_id in self.pending:
            future = self.pending[event.stream_id][0]
            if not future.done(): future.set_exception(RuntimeError(f'H3 stream reset: {event.error_code}'))
        for event in self.http.handle_event(event):
            if isinstance(event, (HeadersReceived, DataReceived)) and event.stream_id in self.pending:
                future, headers, body = self.pending[event.stream_id]
                if isinstance(event, HeadersReceived): headers.update(event.headers)
                else: body.extend(event.data)
                if event.stream_ended and not future.done(): future.set_result((headers, bytes(body)))
    async def request(self, path, method='GET', body=b'', headers=()):
        stream = self._quic.get_next_available_stream_id()
        future = asyncio.get_running_loop().create_future()
        self.pending[stream] = future, {}, bytearray()
        self.http.send_headers(stream, [(b':method', method.encode()), (b':scheme', b'https'), (b':authority', b'localhost'), (b':path', path.encode()), *headers], end_stream=not body)
        if body: self.http.send_data(stream, body, end_stream=True)
        self.transmit()
        try: return await asyncio.wait_for(future, 5)
        finally: del self.pending[stream]

async def exercise(port, secure, cert):
    context = ssl.create_default_context(cafile=str(cert))
    base = f'{"https" if secure else "http"}://localhost:{port}'
    for h2 in ([False, True] if secure else [False]):
        async with httpx.AsyncClient(base_url=base, verify=context, http2=h2, trust_env=False, timeout=5) as client:
            r = await client.get('/plaintext')
            assert r.status_code == 200 and r.text == 'Hello, World!'
            assert r.http_version == ('HTTP/2' if h2 else 'HTTP/1.1'), r.http_version
            payload = b'x' * 256000
            r = await client.post('/echo', content=payload)
            assert r.status_code == 200 and r.content == payload
            r = await client.post('/json', json={'hello': [1, 2, 3]})
            assert r.json() == {'hello': [1, 2, 3]}
            assert (await client.post('/json', content=b'{', headers={'content-type': 'application/json'})).status_code == 400
            assert (await client.post('/upload', files={'file': ('test.txt', b'abc' * 10000)})).text == '30000'
            r = await client.get('/events')
            assert r.headers['content-type'] == 'text/event-stream' and 'data:second\n\n' in r.text
            r = await client.get('/large', headers={'accept-encoding': 'gzip'})
            assert r.headers['content-encoding'] == 'gzip' and r.content == b'x' * 4096
            assert (await client.get('/users/42')).text == '42'
            assert (await client.get('/users/no')).status_code == 400
            assert (await client.head('/plaintext')).content == b''
            r = await client.head('/%ff')
            assert r.status_code == 400 and r.content == b''
            assert 'Origin' in (await client.get('/plaintext')).headers['vary']
            assert (await client.get('/plaintext', headers={'origin':'https://evil.example'})).status_code == 403
            r = await client.options('/plaintext', headers={'origin':'https://example.com', 'access-control-request-method':'POST', 'access-control-request-headers':'content-type'})
            assert r.status_code == 204 and r.headers['access-control-allow-origin'] == 'https://example.com'
            results = await asyncio.gather(*(client.post('/echo', content=bytes([i]) * 128000) for i in range(16)))
            assert all(r.status_code == 200 and r.content == bytes([i]) * 128000 for i, r in enumerate(results))
            print(f'{r.http_version} {"TLS1.3" if secure else "plain"}: extraction, JSON, upload, SSE, gzip, CORS, HEAD, 16 concurrent uploads PASS', flush=True)
    context.set_alpn_protocols(["http/1.1"])
    async with websockets.connect(f'{"wss" if secure else "ws"}://localhost:{port}/ws', ssl=context if secure else None, proxy=None) as ws:
        await ws.send('hello'); assert await ws.recv() == 'hello'
        await ws.send(b'x' * 60000); assert await ws.recv() == b'x' * 60000
    print('WebSocket text/binary echo PASS', flush=True)
    if secure:
        config = QuicConfiguration(is_client=True, alpn_protocols=['h3'], server_name='localhost')
        config.load_verify_locations(cafile=str(cert))
        async with connect('127.0.0.1', port, configuration=config, create_protocol=H3Client) as client:
            headers, body = await client.request('/plaintext')
            assert headers[b':status'] == b'200' and body == b'Hello, World!'
            results = await asyncio.gather(*(client.request('/echo', 'POST', bytes([i]) * 128000) for i in range(16)))
            assert all(h[b':status'] == b'200' and b == bytes([i]) * 128000 for i, (h, b) in enumerate(results))
            headers, body = await client.request('/json', 'POST', b'{"a":42}', [(b'content-type', b'application/json')])
            assert json.loads(body) == {'a':42}
            headers, body = await client.request('/plaintext', 'HEAD')
            assert headers[b':status'] == b'200' and not body
            headers, body = await client.request('/events')
            assert b'data:second\n\n' in body
            headers, body = await client.request('/users/no')
            assert headers[b':status'] == b'400'
            for lengths in [(b'1',), (b'99',), (b'3', b'99'), (b'+3',)]:
                try:
                    await client.request('/echo', 'POST', b'abc', [(b'content-length', value) for value in lengths])
                    raise AssertionError('malformed length received clean success')
                except RuntimeError as error:
                    assert str(error) == 'H3 stream reset: 270', error  # H3_MESSAGE_ERROR
        print('HTTP/3 verified QUIC: 16 concurrent uploads, flow control, JSON, HEAD, SSE, errors PASS', flush=True)

async def main():
    ART.mkdir(parents=True, exist_ok=True)
    cert, key = ART/'cert.pem', ART/'key.pem'
    subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(key),'-out',str(cert),'-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost,IP:127.0.0.1'], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    executable = os.environ.get('MAXIMUS_EXAMPLE', str(ROOT/'target/debug/examples/features'))
    for secure in (False, True):
        env = dict(os.environ)
        if secure: env.update(MAXIMUS_CERT=str(cert), MAXIMUS_KEY=str(key))
        process = subprocess.Popen([executable, '127.0.0.1:0'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
        try:
            line = process.stdout.readline().strip()
            assert line.startswith('listening '), line
            port = int(line.rsplit(':', 1)[1])
            await exercise(port, secure, cert)
        finally:
            process.communicate('\n', timeout=15)
            assert process.returncode == 0
    print('All independent client checks passed.')

if __name__ == '__main__': asyncio.run(main())
