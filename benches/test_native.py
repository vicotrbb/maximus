"""Exercise native controller cleanup and failure preservation with real child processes."""

import importlib.util
import json
import os
import pathlib
import tempfile
import types
import unittest

spec = importlib.util.spec_from_file_location(
    "native_runner", pathlib.Path(__file__).with_name("run-native.py")
)
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)

SERVER = '''#!/usr/bin/env python3
import http.server, os, pathlib
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Length", "13")
        self.end_headers()
        self.wfile.write(b"Hello, World!")
    def log_message(self, *args): pass
host, port = os.environ["ADDRESS"].split(":")
server = http.server.HTTPServer((host, int(port)), Handler)
pathlib.Path(__file__).with_name("server.pid").write_text(str(os.getpid()))
print("ready", flush=True)
server.serve_forever()
'''

CLIENT = '''#!/usr/bin/env python3
import json, os, pathlib, sys, urllib.request
with urllib.request.urlopen(os.environ["URL"] + os.environ["PATHNAME"]) as response:
    assert response.status == 200 and response.read() == b"Hello, World!"
failed = pathlib.Path(__file__).with_name("fail-client").exists()
values = {
    "http_reqs": {"count": 1, "rate": 1},
    "iteration_starts": {"count": 1}, "iterations": {"count": 1},
    "http_req_duration": {"med": 1, "p(99)": 1, "p(99.9)": 1},
    "http_req_failed": {"passes": 0}, "checks": {"fails": int(failed)},
}
pathlib.Path(os.environ["RESULT"]).write_text(json.dumps({
    "metrics": {key: {"values": value} for key, value in values.items()}
}))
sys.exit(int(failed))
'''


class NativeControllerTests(unittest.TestCase):
    def fixture(self, root):
        server = root / "maximus-comparison"
        client = root / "k6"
        for path, source in ((server, SERVER), (client, CLIENT)):
            path.write_text(source)
            path.chmod(0o755)
        options = types.SimpleNamespace(
            k6=client, duration="0.1s", vus=1, workers=1, rate=0, backend="poll"
        )
        return server, options

    def assert_server_stopped(self, root):
        pid = int((root / "server.pid").read_text())
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)

    def test_checked_requests_and_server_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            server, options = self.fixture(root)
            row = native.run_trial(
                options, server, "maximus", "plain", "/plaintext", 1024,
                root / "passing", None,
            )
            self.assertEqual(row["requests"], 1)
            self.assertEqual(row["iterations_completed"], 1)
            self.assertEqual(row["exit"], 0)
            self.assertGreater(row["cpu_us_per_request"], 0)
            self.assert_server_stopped(root)

    def test_failed_warmup_preserves_raw_results_and_stops_server(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            server, options = self.fixture(root)
            (root / "fail-client").touch()
            with self.assertRaises(ValueError):
                native.run_trial(
                    options, server, "maximus", "plain", "/plaintext", 1024,
                    root / "failing", None,
                )
            evidence = [json.loads(path.read_text()) for path in root.glob("*.json")]
            self.assertTrue(any(item.get("metrics", {}).get("checks", {}).get("values", {}).get("fails") == 1 for item in evidence))
            self.assert_server_stopped(root)


if __name__ == "__main__":
    unittest.main()
