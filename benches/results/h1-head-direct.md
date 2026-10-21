# Direct parsed-head return: rejected for minimal-artifact growth

**Decision: REJECTED before a CPU campaign.** Returning the parsed `Head`
directly from the H1 header scanner grows the minimal hello artifact's actual
`.text` section by **3,840 bytes**, despite a smaller conformance GNU text
aggregate. The strict no-regression size gate rejects this tradeoff. No CPU or
allocation measurement is assigned to this direct-return variant.

Only `crates/http1/src/lib.rs` differs between `h1-head-baseline-source` and
`h1-head-source`. The prototype moves parsing into the scanner while borrowing
the completed header prefix, then drains that prefix. It attempts to eliminate
the intermediate collected header `Vec`, returning an owned parsed head to the
connection loop. This is a source-level opportunity, not a measured allocation
saving. The experiment remains in frozen artifacts; root reports that product
code retains the preceding borrowed-timer variant.

## Matched footprint observations

The hello and conformance controls are rebuilt for both frozen sources. GNU
`size` text and the actual ELF `.text` section are distinct measurements:

| Artifact measure | Baseline | Direct-return candidate | Change |
| --- | ---: | ---: | ---: |
| Hello actual `.text` section | 618,328 | 622,168 | +3,840 |
| Hello GNU text aggregate | 827,585 | 831,473 | +3,888 |
| Hello data | 27,656 | 27,680 | +24 |
| Hello file bytes | 1,279,584 | 1,279,800 | +216 |
| Hello stripped file bytes | 922,400 | 922,400 | 0 |
| Conformance GNU text aggregate | 2,146,608 | 2,146,076 | −532 |
| Conformance data | 59,344 | 59,368 | +24 |
| Conformance file bytes | 2,953,664 | 2,953,840 | +176 |
| Conformance stripped file bytes | 2,233,184 | 2,233,184 | 0 |

The larger conformance artifact's text decrease is retained evidence. It does
not offset minimal-artifact code growth under the user's requirement.
Identical stripped file lengths do not establish identical bytes or unchanged
executable code. BSS remains 2,488 bytes for hello and 2,560 for conformance.

The separate H1 public serve-future layout probe constructs, without polling,
the compat/native futures. Both sources record **4,328/4,352 bytes**, so this
representation change supplies no observed future-storage reduction. These
are H1 future types, separate from earlier H2 layout measurements.

## Correctness and exact artifacts

The candidate's native H1 control suite records **19 passed, zero failed**,
including fragmented/pipelined header metadata, buffer reuse, body framing,
trailers, unread bodies, CONNECT, and timeout cases. The new fragmented-header
pipeline control also passed against the root baseline before the experiment.
These are the recorded controls, rather than complete performance acceptance.

| Frozen input | SHA-256 |
| --- | --- |
| Baseline H1 source | `f7c14425b0cc7bf890338d0b2dfd1501211e7ca3769b6744d1f595278d98d586` |
| Direct-return H1 source | `098f44a21891f306ce574a7b5a1034f059932539146568ba4dae7709511e82e9` |
| Baseline hello executable | `b841917e80730cf93b83ac3f6fca9ba7725a307bba5818e19acbfe3b4f113a13` |
| Candidate hello executable | `4841baf7212f499e38bc296bd7eeaee3816cc7efe7769b58ffacb92e6aa91d0f` |

The exact manifests are `h1-head-baseline-manifest.json` and
`h1-head-manifest.json`. Footprints come from
`verify-size-h1-head-baseline.log`, `verify-size-h1-head.log`, and
`head-direct-hello-sections.log`; layout comes from
`head-layout-h1-head-baseline.log` and `head-layout-h1-head.log`, with
`head-layout-probe` source retained. Native controls are in
`h1-head-native-controls.log` and `h1-head-baseline-control.log`.

Memory-transport probes were built with baseline SHA
`30ff4f5201cfa0b4007f30efdd9e5178d0189a071b2177cf89ec38e7b7bee02d`
and candidate SHA
`9f6011a94223486883ea147a2743368ed1307af45228152ae55550eccd997a72`.
**No CPU campaign ran for this direct version.** Build completion supplies no
timed observation, throughput, latency, allocation-counter, or RSS result.

[Maintained evidence](h1-head-direct-evidence.json) embeds source manifests,
the one-file product patch, raw footprint/control/layout/build logs and the
layout probe, with every input hash. The
[offline reconciler](../analyze-h1-head-rejection.py) verifies the one-file scope,
source hashes, exact artifact rows, unchanged future sizes, control outcomes
and built-but-unmeasured probe identities. A subsequent `head_len` alternative
is a separate experiment; its results do not change this rejection.
