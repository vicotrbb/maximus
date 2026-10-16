# Contributing

Keep product code independent of benchmark and fuzz dependencies. Prefer maintained protocol/runtime APIs over new unsafe code or a second parser. Feature additions need a failing behavior test first, an integration check at the relevant boundary, and an update to the feature matrix and guide.

Run `scripts/check.sh` and `scripts/test-linux.sh`. The latter requires Docker with io_uring access. For networking changes also run the independent interop and adversarial tests described in `docs/qualification.md`. Privileged BPF tests are separate and opt-in. Never require BPF privileges for an ordinary build.

For performance changes, preserve baseline and candidate raw results, use identical workloads and CPU placement, repeat trials, and report medians plus tail latency, errors, dropped iterations, CPU and memory. A lower latency under a lower offered load is not an improvement. Do not commit build outputs, private keys, profiler dumps or generated fuzz corpora.

HTTP/3 changes must also run the retained upstream tests with `cargo test --manifest-path vendor/h3/Cargo.toml --all-features --locked` and `scripts/check-downstream.sh`. Keep changes to vendored code minimal and listed in `vendor/h3/PATCHES.md`; the copy exists only for validation defects that cannot be repaired after decoding.
