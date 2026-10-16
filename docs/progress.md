# Maximus progress

Plan: docs/superpowers/plans/2026-10-15-maximus.md

- Original specification read in full; copied without changes.
- Host Rust 1.96.0. Docker Linux aarch64 available with 3 CPUs.
- Goal active. No product capability or performance claim verified yet.
- Architecture decision: use maintained Rust protocol engines and Compio beneath Maximus-owned worker orchestration, limits and API. This reduces custom unsafe and protocol code; dependency behavior remains subject to integration tests.
- User supplied implementation authorization and full specification; execute continuously in the new isolated maximus directory.

## Foundation checkpoint

- Added memory, io, net and runtime crates with no owned unsafe code.
- Host suite: 12 tests passed. Host clippy with warnings denied passed.
- Linux Docker suite: 13 tests passed including explicit io_uring, epoll, CPU pinning, local non-Send tasks and shutdown notification. Output: artifacts/foundation-linux.log.
- Fixed-capacity slab rejects stale keys; buffer cache discards oversized returned buffers. The buffer cache does not itself bound caller-owned growth; server body and connection budgets must do so.
- NUMA topology discovery and first-touch allocation follow worker pinning. eBPF attachment and kernel optimization measurements remain open.
- Core API tests observed missing API failures before implementation. Core integration now in progress.
