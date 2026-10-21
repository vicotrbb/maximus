# Linux qualification bundle

These patches are independent experiments, not accepted optimizations. Each
applies to the corrected source exported by `benches/export-qualification.py`.
They preserve rejected-final CONNECT closure and registered 101 upgrade headers.
Do not apply them together before each individual experiment qualifies; a later
combination requires its own fresh checks and measurements.

Export from the current research checkout into a new directory:

```sh
python3 benches/export-qualification.py artifacts/qualification-bundle
```

The exporter captures source bytes, both workspaces' lockfiles, vendored H3,
regression tests, benchmark tools, and these three patches. It applies each
patch in a temporary restored tree and verifies that exactly its named library
changes. `manifest.json` records every source hash, archive hash, export checkout
HEAD and candidate-library hashes. It does not claim compilation,
protocol correctness or measured performance for a patched variant.

Use the resulting bundle on Linux with Docker, Python 3, Git and at least three
available CPU IDs, including 0, 1 and 2. An idle host improves measurement
conditions. Local Docker execution was authorized for the subsequent
[k6 campaign](../results/local-docker-k6.md); its shared-host limits remain
recorded. Do not stop other workloads or change host settings to obtain
exclusivity. Verify the archive against `manifest.json`
before extraction. The bundle excludes `.git`, build caches, raw measurements,
TLS keys and ignored files.

Extract into a new directory. Initialize Git there and make a local baseline
snapshot commit so the existing runner can record Git provenance. That commit
identifies the exported working tree. The manifest's `original_head` identifies
the checkout HEAD at export time. The historical performance baseline is
`original_performance_reference.repository_commit`, currently `b5bb664bd6faf9b417cd8af5235ec12e7f0037de`.
Keep the manifest with all results. No remote or push is needed.

## Build and correctness stage

Complete all builds, tests, allocation diagnostics and footprint collection
before starting any timing campaign. Use the same compiler image ID, architecture,
release settings and container source path `/work` for every variant. Save image
IDs and `rustc -Vv`; an x86 Linux campaign is separate from the earlier ARM VM
measurements. Do not pool them.

Build the baseline and then one patch at a time. Run the repository checks and
independent interoperability suite for each source. Record the registered
CONNECT 200/101 matrix, rejected-CONNECT controls, body-framing/pipeline controls,
and H2 wire/lifecycle controls. Restore the patch before preparing the next one.
Verify all library hashes against the manifest, applying the selected
candidate hash override, before and after each check.

When changing source under the shared benchmark Cargo target, explicitly clean
release artifacts for the owned packages before rebuilding:

```sh
cargo clean --release --manifest-path benches/baselines/Cargo.toml \
  -p maximus-core -p maximus-http1 -p maximus-http2 -p maximus \
  -p maximus-comparison
cargo build --release --locked --manifest-path benches/baselines/Cargo.toml --bins
```

Run these commands inside the fixed `/work` container mapping used by
`benches/README.md`. Copy each completed executable to a new immutable directory
in `maximus-bench-target` before switching source. Refuse an existing export
path, record SHA-256, and collect the allocation/component executables separately.
Keep minimal hello and conformance footprint measurements distinct from the
larger comparison fixture. Include future sizes, stripped and unstripped file
sizes, text/data/BSS, and allocation calls/bytes.

## Original performance reference

`original-http1.rs` is the H1 library from research starting commit
`b5bb664bd6faf9b417cd8af5235ec12e7f0037de`, with identity recorded in
`original-reference.json`. The other corrected-baseline production source files
already match that commit. Build this source as a separate benchmark reference,
then restore the corrected baseline before applying a candidate patch. It is
not an admissible final implementation: it retains the known rejected-CONNECT
defect and is expected to fail the new rejection controls.

The final combination must also be compared against this original reference on
common valid workloads and linked footprint. Improvement only against the
corrected baseline must not hide a net regression relative to the starting
framework. Keep the original-reference, corrected-baseline and isolated-candidate
identities distinct. Never count the original's incorrectly processed requests
as useful throughput or pool its earlier measurements into the new campaign.

## Timing stage

First run repeated same-binary Maximus, Hyper and Axum controls with the chosen
CPU affinity. Retain throughput, p99/p99.9, CPU/request, RSS/peak RSS, client CPU,
errors, dropped/incomplete iterations and environment metadata. Unstable controls
prevent performance acceptance; do not subtract comparator variation from
candidate results or discard adverse trials.

For each isolated candidate, use fresh labels and matched source-order rotation.
The existing `compare-ownership.py` covers plain H1 dynamic/static/error/echo
requests and TLS H2 error/large echo requests. Use k6 for plain H1, TLS H1 and
TLS H2, with repeated trials and one certificate within each TLS comparison.
It does not cover TLS H1 performance: add that matched cohort through `run.py
--protocol h1` before broader acceptance. Include sustained load, connection
churn, streaming/backpressure and concurrent H2 streams where relevant to the
changed component. Define the workload plan and statistical decision procedure
before inspecting candidate timing results.

Response checks or a successful runner exit are functional outcomes, not
performance acceptance. Inspect every workload and resource metric against
its paired baseline, including adverse observations. An unresolved result stays
unqualified. No source is promoted merely because it allocates less, has a
smaller future or wins one workload. Capture profiles only in separate runs.
