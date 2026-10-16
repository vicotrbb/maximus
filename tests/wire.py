#!/usr/bin/env python3
"""Deterministic raw-wire mutation and disconnect stress; no third-party client."""
import concurrent.futures, os, pathlib, random, re, socket, subprocess
root = pathlib.Path(__file__).resolve().parents[1]
process = subprocess.Popen([os.environ.get('MAXIMUS_HELLO', str(root/'target/debug/examples/hello')), '127.0.0.1:0'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
try:
    line = process.stdout.readline()
    port = int(re.search(r'127.0.0.1:(\d+)', line).group(1))
    def send(data):
        with socket.create_connection(('127.0.0.1', port), timeout=2) as stream:
            stream.sendall(data)
            stream.shutdown(socket.SHUT_WR)
            output = bytearray()
            try:
                while chunk := stream.recv(4096):
                    output.extend(chunk)
                    assert len(output) < 65536
            except ConnectionResetError:
                pass  # Parser rejection may reset unread bytes; the worker must survive.
            return bytes(output)
    rng = random.Random(196)
    seed = b'GET /users/42 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n'
    corpus = [seed[:i] for i in range(len(seed))]
    for _ in range(2000):
        data = bytearray(seed)
        for _ in range(rng.randint(1, 8)):
            data[rng.randrange(len(data))] = rng.randrange(256)
        corpus.append(data)
    with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
        list(pool.map(send, corpus))
    assert process.poll() is None
    # A final valid request proves the workers still accept and dispatch traffic.
    with socket.create_connection(('127.0.0.1', port), timeout=2) as stream:
        stream.sendall(seed)
        output = bytearray()
        while chunk := stream.recv(4096): output.extend(chunk)
    assert b'200 OK' in output and output.endswith(b'user 42\n'), output
    print(f'{len(corpus)} raw-wire mutations/disconnects, 16 concurrent clients, post-stress health PASS')
finally:
    process.communicate('\n', timeout=10)
    assert process.returncode == 0
