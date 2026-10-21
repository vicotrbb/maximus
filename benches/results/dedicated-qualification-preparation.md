# Dedicated-host preparation

This historical preparation predates approval to run locally in Docker.
The subsequent [local k6 report](local-docker-k6.md) records that completed
campaign and supersedes this document's execution-target requirement.

**No performance candidate is accepted or retained in product code.** This
preparation supplies independently patched sources for future qualification;
it supplies no new throughput, latency, CPU/request or memory-performance result.

The exported baseline contains the corrected rejected-final CONNECT behavior,
including preservation of registered 101 upgrade headers. Three separate patches
change borrowed-error conversion, empty-H1 completion tracking, or the H2 timer.
Each applies to a restored baseline, changes exactly one named library, and
preserves the corrected CONNECT condition. Earlier combined-source measurements
are not assigned to these new isolated candidates.

The final archive contains 197 files: product/tests, manifests and lockfiles,
vendored source, qualification documentation, benchmark tools, and the patches.
It excludes Git metadata, build caches, ignored raw results and TLS credentials.
Two exports produce identical archive bytes. Independent extraction verifies
all file hashes, all three patch overrides, and restoration between patches.
The archive SHA-256 is
`7829012a096a5676a68bf560fff240a2c0f6c15552697e9483a57338563405d8`.

Review found that the initial patch validator could miss a newly added file.
The negative control reproduces that acceptance, then verifies that the fixed
validator rejects the same extra-file patch before creating output. The final
validator checks the complete path inventory and rejects symlinks before
checking content differences. Root product sources were not changed by this
packaging work.

## Candidate correctness checks

All three candidates first pass Clippy with warnings denied for the workspace,
all features and all targets, plus 152 Linux all-feature test executions each.
They then pass the full `scripts/check.sh` and independent interoperability suite.
The full check includes feature combinations, fuzz-target compilation and docs:


| Isolated candidate | Passing test executions | Failed | Ignored |
| --- | ---: | ---: | ---: |
| Borrowed errors | 180 | 0 | 1 |
| Empty-H1 tracking | 180 | 0 | 1 |
| Borrowed H2 timer | 180 | 0 | 1 |

The ignored control requires privileged BPF access. These are repeated executions
for distinct sources, rather than 540 unique tests. Rust 1.96.1 on aarch64 Linux
was used. Owned dev artifacts were cleaned between sources under the same `/work`
container path. No timed campaign ran concurrently.

Checks ran against the recorded staging bundles. The final bundle adds packaging
and original-reference inputs; every compiled product/vendor/example/fuzz input,
Cargo manifest/lockfile, and candidate-library hash matches the checked staging
source. The [evidence](dedicated-qualification-preparation.json) preserves both
manifests, exact commands, image/compiler identity, log hashes, observed exits,
source equivalence and the extra-file control. Each candidate also passes independent H1 plain/TLS, H2 TLS, WebSocket and H3
QUIC checks, including 2,062 wire mutations/disconnects, concurrent clients and
post-stress health. Initial harness target-mount/fuzz-output failures were fixed
without source edits; successful retries and earlier failure logs remain separate.
These functional results do not establish any performance gate.

The archive also includes **b5bb664 production source with the current shared
qualification harness** as an original-performance reference. It is not an exact
historical checkout or an admissible final implementation: the original H1
library retains the known rejected-CONNECT defect. Its bytes match the named Git
commit and carry a separate library-hash override. Final combined performance
and footprint must pass fresh comparisons against both this original reference
and the corrected baseline on common semantically equivalent valid workloads.
Its erroneous request processing is never useful throughput. No historical
timings are pooled, and no fresh original-reference executable has been qualified.

A post-check host snapshot still observes an external coverage/test workload.
That is current environment context, not a cause assigned to earlier timing
variation. No external process or host setting was changed.

Use the [qualification runbook](../qualification-candidates/README.md) and
[exporter](../export-qualification.py) to prepare a new bundle from current
source. An idle, dedicated Linux target is still required. Begin with inventory
and stable same-binary controls; retain every failure and adverse measurement.
Complete matched Hyper/Axum socket coverage, including TLS H1, before accepting
any candidate. A later combination needs its own qualification.
