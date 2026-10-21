# Same-binary H2 control after cache cleanup

**Decision: completed A/A control; host stability remains unestablished.** All
eighteen observations pass response/iteration validation, yet the unchanged
executable exhibits substantial throughput, CPU/request, and tail variation.
This diagnostic supplies no candidate performance acceptance and does not
establish cache cleanup as the cause of any difference.

The control runs six repeats each of Maximus, Hyper, and Axum from the exact
same comparison executable:
`be05e0aea314cfdda839441c4c3c923e4cae9c8100f4836f382d7ca1e703e8f2`.
The frozen product commit is `b5bb664bd6faf9b417cd8af5235ec12e7f0037de`,
identified by `core-h2-baseline-manifest.json` and its completed build log.
The frozen comparator lock contains Hyper 1.12.0, Axum 0.8.9, Tokio 1.53.3,
and rustls 0.23.46. Each engine is an argument to that executable; none is the
borrowed-timer candidate. The environment's dirty-checkout flag describes the
working checkout, while the separately captured executable hash fixes this
control's artifact identity.

Every observation uses TLS H2 `/echo`, a 128 KiB body, 64 closed-loop VUs,
one server worker on CPU 0, client CPUs 1,2, a three-second warmup, and a
five-second measured client setting. Offered-rate limiting and periodic memory
sampling are disabled. Servers use private network namespaces, with their
clients sharing the respective namespace. The shared Linux ARM VM has three
CPUs. Order rotates by repeat:

| Repeat | Engine order |
| --- | --- |
| 0, 3 | Maximus → Hyper → Axum |
| 1, 4 | Hyper → Axum → Maximus |
| 2, 5 | Axum → Maximus → Hyper |

Root directly observed the invocation's zero exit in command session 58883 and
reports no owned builds or other owned loads during the run. The environment
inventory includes unrelated containers; their presence alone establishes
neither workload activity nor interference. This is a separate invocation after
cache cleanup, not a controlled experiment isolating cleanup's effect.

## Reconciliation and observed ranges

The [offline analyzer](../analyze-post-cache-aa-h2.py) validates all eighteen
rows, their rotating order and workload settings, raw warmup/measurement checks,
completed iterations, exits, CSV/JSON metrics, client CPU arithmetic, and the
captured executable, workload, image, and certificate identities. Every response
passes the explicit status/body/protocol check. Counts are **510,767 measured
responses** and **292,082 warmup responses**, with no incomplete iterations,
drops, or failed checks. Raw reported client durations are 5,002.117–5,012.965 ms
for measurement and 3,002.388–3,023.790 ms for warmup.

| Engine | RPS range | CPU μs/request range | p99 ms range | p99.9 ms range |
| --- | ---: | ---: | ---: | ---: |
| Maximus | 3,360.122–5,121.707 | 172.534–226.301 | 25.512–70.238 | 38.593–141.093 |
| Hyper | 3,442.013–6,831.908 | 118.182–162.671 | 25.580–94.584 | 37.286–119.555 |
| Axum | 5,383.797–6,825.533 | 119.381–131.080 | 26.489–42.544 | 40.571–97.420 |

CPU snapshot intervals span **5.43358025001362–6.517835750011727 seconds**.
The 45-second interval observed in the
[aborted borrowed-timer campaign](borrowed-timer.md) does not recur here;
the remaining variation does not establish a stable performance environment.
These ranges describe this invocation, without pooling prior cohorts.

## Adjacent same-binary pairs

Pairing is fixed as repeats 0→1, 2→3, and 4→5 within each engine. Percentages
mean later/earlier differences from an identical engine executable. They are
A/A diagnostic variation, not candidate gains. All nine pairs remain present:

| Engine | Repeats | RPS | CPU/request | p99 | p99.9 |
| --- | --- | ---: | ---: | ---: | ---: |
| Maximus | 0→1 | −21.03% | +4.04% | +149.59% | +265.59% |
| Maximus | 2→3 | +17.08% | −5.35% | −48.13% | −32.96% |
| Maximus | 4→5 | −22.57% | +25.64% | +3.08% | +14.35% |
| Hyper | 0→1 | −0.37% | −0.07% | +1.57% | −6.86% |
| Hyper | 2→3 | −4.99% | +6.03% | +33.71% | +51.79% |
| Hyper | 4→5 | −49.39% | +36.40% | +265.12% | +127.99% |
| Axum | 0→1 | −0.34% | +0.19% | −9.03% | −6.87% |
| Axum | 2→3 | −5.50% | +4.55% | −1.87% | −18.31% |
| Axum | 4→5 | −19.99% | +5.13% | +55.34% | +66.20% |

## Provenance and limits

Captured immutable image IDs are k6
`sha256:be2a9ceb1b6ffc573277af4157135882a6d6433968b41e858ab02c5fd5847532`
and Rust
`sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663`.
The captured certificate SHA-256 is
`d2bb8d5f5222cc6f96483eda99ae6bbc6bb2eef1a01b7ac22d46fc0c296d3f8d`.
One certificate is generated for the single runner invocation and mounted for
all eighteen observations. Image/certificate identities are captured once per
invocation, rather than independently verified for every engine container.

[Evidence](post-cache-aa-h2-evidence.json), [all trials](post-cache-aa-h2-trials.csv),
and [all pair changes](post-cache-aa-h2-pairs.csv) preserve the raw observations,
warmup counts, exact environment, source/build references, hashes, summaries,
and analyzer. Raw server before/after CPU snapshots are not retained: the
analyzer checks internal CSV arithmetic and the recorded client cgroup counter,
without independently reconstructing server CPU usage. This A/A variation does
not prove the cause of earlier adverse results, erase them, or qualify the
borrowed timer's socket performance.
