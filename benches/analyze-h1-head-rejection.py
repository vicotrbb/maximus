#!/usr/bin/env python3
"""Preserve the direct Head-return footprint rejection; no workloads/builds."""

import difflib
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "artifacts/performance-2026-10-20"
OUT = ROOT / "benches/results"
inputs = {}


def read(name):
    path = ART / name
    inputs[str(path.relative_to(ROOT))] = path.read_text()
    return inputs[str(path.relative_to(ROOT))]


def require(condition, message):
    if not condition:
        raise ValueError(message)


manifests = {v:json.loads(read(f"{name}-manifest.json")) for v,name in [("baseline","h1-head-baseline"),("candidate","h1-head")]}
require({k for k in set(manifests["baseline"]) | set(manifests["candidate"]) if manifests["baseline"].get(k) != manifests["candidate"].get(k)} == {"crates/http1/src/lib.rs"}, "unexpected source scope")
sources = {v:read(f"{name}-source/crates/http1/src/lib.rs") for v,name in [("baseline","h1-head-baseline"),("candidate","h1-head")]}
for v in sources:
    require(hashlib.sha256(sources[v].encode()).hexdigest() == manifests[v]["crates/http1/src/lib.rs"], "frozen H1 source differs from manifest")
footprints = {}
for variant, name in [("baseline","h1-head-baseline"),("candidate","h1-head")]:
    log = read(f"verify-size-{name}.log")
    require(log.count("Finished `release` profile") == 2, "incomplete footprint builds")
    footprints[variant] = {}
    for artifact in ["hello","conformance"]:
        digest = re.findall(r"^([0-9a-f]{64})\s+\S*/" + artifact + "$", log, re.MULTILINE)
        fields = re.findall(r"^\s*(\d+)\s+(\d+)\s+(\d+)\s+\d+\s+[0-9a-f]+\s+\S*/" + artifact + "$", log, re.MULTILINE)
        sizes = re.findall(r"^\S*/" + artifact + r"(\.stripped)? (\d+)$", log, re.MULTILINE)
        require(len(digest) == len(fields) == 1 and len(sizes) == 2, "ambiguous footprint rows")
        footprints[variant][artifact] = dict(sha256=digest[0], gnu_text_bytes=int(fields[0][0]), data_bytes=int(fields[0][1]), bss_bytes=int(fields[0][2]), file_bytes={suffix:int(size) for suffix,size in sizes})
sections = read("head-direct-hello-sections.log")
hello_text = [int(x) for x in re.findall(r"^\.text\s+(\d+)\s+\d+$", sections, re.MULTILINE)]
require(hello_text == [618328,622168], "unexpected actual hello text sections")
layouts = {}
for name in ["h1-head-baseline","h1-head"]:
    text = read(f"head-layout-{name}.log")
    layouts[name] = {key:int(value) for key,value in re.findall(r"^(compat_serve_future_bytes|native_serve_future_bytes),(\d+)$", text, re.MULTILINE)}
require(all(v == {"compat_serve_future_bytes":4328,"native_serve_future_bytes":4352} for v in layouts.values()), "unexpected H1 future storage")
for name in ["src/main.rs","Cargo.toml","Cargo.lock"]:
    read("head-layout-probe/" + name)
controls = read("h1-head-native-controls.log")
baseline_control = read("h1-head-baseline-control.log")
require("19 passed; 0 failed; 0 ignored" in controls and "1 passed; 0 failed; 0 ignored" in baseline_control, "native control outcome differs")
memory_binaries = {}
for v,name in [("baseline","h1-head-baseline"),("candidate","h1-head")]:
    log = read(f"memory-http1-{name}-build.log")
    digest = re.findall(r"^([0-9a-f]{64})\s+\S*/probe$", log, re.MULTILINE)
    require(len(digest) == 1 and "Finished `release` profile" in log, "incomplete unmeasured memory probe build")
    memory_binaries[v] = digest[0]
require(memory_binaries == {"baseline":"30ff4f5201cfa0b4007f30efdd9e5178d0189a071b2177cf89ec38e7b7bee02d", "candidate":"9f6011a94223486883ea147a2743368ed1307af45228152ae55550eccd997a72"}, "unexpected unmeasured direct-version probes")
result = dict(status="REJECTED direct Head-return artifact before CPU campaign", source_scope="Only crates/http1/src/lib.rs differs between frozen manifests; root product unchanged by this experiment", source_manifests=manifests, product_patch="".join(difflib.unified_diff(sources["baseline"].splitlines(keepends=True), sources["candidate"].splitlines(keepends=True), fromfile="a/crates/http1/src/lib.rs", tofile="b/crates/http1/src/lib.rs")), footprints=footprints, actual_hello_text_section_bytes=dict(baseline=hello_text[0],candidate=hello_text[1],delta=hello_text[1]-hello_text[0]), h1_serve_future_storage=layouts, native_controls=dict(candidate_passed=19,candidate_failed=0,baseline_fragmented_pipeline_control_passed=1), built_but_unmeasured_memory_binaries=memory_binaries, timed_observations_assigned=0, limits=["Minimal hello actual .text and GNU aggregate text grow; conformance GNU text decreases, but does not satisfy the strict no-regression size gate.", "Identical stripped file lengths are not evidence of identical bytes, unchanged executable text, or zero code growth.", "Public H1 serve futures are constructed but not polled by the layout probe; its storage is not CPU performance.", "No direct-version CPU campaign ran, so no throughput, latency, allocation or RSS result is assigned.", "The head_len alternative is a separate artifact/experiment and does not change this direct-version rejection."], embedded_artifact_text=inputs, artifact_sha256={name:hashlib.sha256(text.encode()).hexdigest() for name,text in inputs.items()}, analysis_script=Path(__file__).read_text())
(OUT / "h1-head-direct-evidence.json").write_text(json.dumps(result,indent=2)+"\n")
print(json.dumps({"footprints":footprints,"actual_hello_text_section_bytes":result["actual_hello_text_section_bytes"],"timed_observations_assigned":0},indent=2))
