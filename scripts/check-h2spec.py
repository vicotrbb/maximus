#!/usr/bin/env python3
"""Run the pinned legacy h2spec suite and report RFC 9113 policy differences explicitly."""
import argparse
import os
import pathlib
import subprocess
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("h2spec", type=pathlib.Path)
parser.add_argument("--tls", action="store_true")
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[1]
art = root / "artifacts/h2spec"
art.mkdir(parents=True, exist_ok=True)
label = "tls" if args.tls else "plain"
env = dict(os.environ)
for key in ("MAXIMUS_CERT", "MAXIMUS_KEY", "MAXIMUS_H2C"):
    env.pop(key, None)
if args.tls:
    cert, key = art / "cert.pem", art / "key.pem"
    subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                    "-keyout", str(key), "-out", str(cert), "-days", "1",
                    "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost"],
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    env.update(MAXIMUS_CERT=str(cert), MAXIMUS_KEY=str(key), SSL_CERT_FILE=str(cert))
executable = env.get("MAXIMUS_CONFORMANCE", str(root / "target/debug/examples/conformance"))
server = subprocess.Popen([executable, "127.0.0.1:0"], env=env, stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, text=True)
try:
    line = server.stdout.readline().strip()
    assert line.startswith("listening "), line
    port = line.rsplit(":", 1)[1]
    xml = art / f"{label}.xml"
    xml.unlink(missing_ok=True)
    command = [str(args.h2spec.resolve()), "--strict", "--host", "localhost", "--port", port,
               "--timeout", "2", "--junit-report", str(xml)]
    if args.tls:
        # h2spec has no CA-file flag; macOS Go uses the system keychain. This
        # local fixture tests TLS protocol behavior. interop.py verifies the CA.
        command.extend(["--tls", "--insecure"])
    with (art / f"{label}.log").open("w") as output:
        result = subprocess.run(command, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=180)
finally:
    server.communicate("\n", timeout=15)
    assert server.returncode == 0, server.returncode

# RFC 9113 §5.1 permits minimal processing of all closed streams. These exact
# RFC 7540-era cases instead demand an error. Do not suppress any other failure.
legacy = {
    ("http2/5.1", "closed: Sends a DATA frame after sending RST_STREAM frame"),
    ("http2/5.1", "closed: Sends a HEADERS frame after sending RST_STREAM frame"),
    ("http2/5.1", "closed: Sends a DATA frame"),
    ("http2/5.1", "closed: Sends a HEADERS frame"),
    ("http2/5.1.1", "Sends stream identifier that is numerically smaller than previous"),
}
assert xml.exists(), f"h2spec did not finish; inspect {art / (label + '.log')}"
cases = ET.parse(xml).findall(".//testcase")
assert len(cases) == 147, f"unexpected h2spec suite: {len(cases)} cases"
assert not any(case.find("skipped") is not None for case in cases), "skipped tests"
failed = {(case.get("package"), case.get("classname")) for case in cases
          if case.find("error") is not None or case.find("failure") is not None}
assert not failed - legacy, f"unexpected failures: {sorted(failed - legacy)}"
assert result.returncode == (1 if failed else 0), f"unexpected h2spec exit {result.returncode}"
print(f"{label}: {len(cases) - len(failed)}/{len(cases)} h2spec cases passed; "
      f"{len(failed)} explicitly reported RFC 7540/9113 closed-stream policy differences; no other failures")
for package, name in sorted(failed):
    print(f"  legacy difference: {package}: {name}")
