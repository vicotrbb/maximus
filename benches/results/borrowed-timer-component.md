# Borrowed absolute timer mechanism probe

The clean standalone probe measures timer construction, one pending poll, and
drop/cancellation inside an actual `compio::runtime::Runtime::block_on`
context. Seven alternating pairs have median relative/borrowed-absolute times
of **55.262 → 25.289 ns/operation**, with the absolute form faster in all seven
pairs. This measures the timer mechanism; it does not qualify HTTP throughput,
tail latency, or the [aborted socket candidate](borrowed-timer.md).

Each mode executes 2,000,000 measured operations after 10,000 warmup operations.
A fixed deadline one hour ahead is chosen separately for each mode. Every
measured operation must return Pending, then drops the registered timer.
Before every mode, an expired deadline must return Ready. Relative runs first
in even rounds and borrowed absolute first in odd rounds. Source assertions
and the reported zero exit cover those controls.

| Round | Relative ns/operation | Borrowed absolute ns/operation | Change |
| --- | ---: | ---: | ---: |
| 0 | 59.771 | 27.709 | −53.64% |
| 1 | 55.262 | 26.613 | −51.84% |
| 2 | 54.680 | 25.289 | −53.75% |
| 3 | 55.660 | 24.974 | −55.13% |
| 4 | 55.157 | 25.157 | −54.39% |
| 5 | 58.759 | 25.267 | −57.00% |
| 6 | 55.186 | 25.309 | −54.14% |

The CSV's `future_bytes` values, **120 → 88**, are `size_of::<F>()` for the
stored input type. They exclude the temporary borrowed Fuse wrapper and the
enclosing async frame. Separately observed compat/native HTTP serve futures
are **1,576 → 1,528** and **2,984 → 2,936 bytes**, each a 48-byte reduction on
the measured target. Those frame measurements come from the independent
baseline and borrowed layout logs, rather than subtracting stored-F sizes.

The exact probe executable hash is
`44e9c32189ddc8bb21055f01936e1010fe13506b8f31d4c0240d6087e132d374`;
the reviewed source hash is
`d185f0a1e05f5b107b20f7f0bb82c574c461a740e051206b62743cde0eda1d66`.
Completed release and Clippy logs are preserved. All 82 dependency packages in
the probe lock match the frozen product lock by name, version, registry source,
and checksum. The only extra package is the standalone probe itself. Relevant
versions are compio 0.19.2, compio-runtime 0.12.6, futures-util 0.3.34, and libc
0.2.190. No product or benchmark workspace dependency was added.

Root directly observed the clean execution's zero exit through the command
tool. The recorded command uses `rust:1.96-bookworm`, `--cpuset-cpus 0`, and the
read-only target volume; stderr records the IoUring driver. No separate probe
environment capture independently records immutable image identity, actual
CPU placement, or compiler version. Other campaign captures identify Rust
1.96.1 in the tagged image, rather than a fresh compiler observation for this
probe. Source/build/lock hashes record provenance but alone cannot prove the
binary was built from a particular source.

Root reports that owned campaigns, builds, and allocation probes had stopped
and incremental cache cleanup increased host free space from about 12 to 19 GB
before this run. That context does not prove the earlier stalls' cause. The
first `timer-probe-borrowed.csv` overlaps a hashing helper and remains
exploratory; its timings are excluded from all statistics here. One process
with seven rounds on one host supplies neither independent repeated-process
uncertainty nor socket qualification.

[Machine-readable evidence](borrowed-timer-component.json) retains every raw
row, pair, dependency version/checksum, relevant file hash, and the exact
reported command. The [offline reconciler](../analyze-borrowed-timer-component.py)
checks fixed source/build identities, locked dependency parity, alternating
order, counts, stored-type sizes, and separately recorded serve layouts.
