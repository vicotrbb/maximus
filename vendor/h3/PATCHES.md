# Temporary strict-field patch

Source: `h3` 0.0.8, upstream revision `22c1aa3f44d1463cd7644c8f654fffc9a6da305c`, [hyperium/h3](https://github.com/hyperium/h3). Published crate SHA-256: `10872b55cfb02a821b69dc7cf8dc6a71d6af25eb9a79662bec4a9d016056b3be`. The unchanged upstream MIT license is included. Only `src/proto/headers.rs` and `src/connection.rs` differ from the published Rust source. The normalized package manifest and upstream tests are retained so this patch can be tested independently. The sibling `vendor/h3-quinn/src` and MIT license come unchanged from the same upstream revision and are required by its test module; they are test support, not a second runtime dependency. Run the upstream suite with all features, as its integration tests import the opt-in public connection traits.

The released decoder discards duplicate/out-of-order pseudo-header evidence before applications can validate it. It also accepts response-only pseudo-fields in requests, pseudo-fields in trailers, connection-specific fields, invalid TE and empty schemes. Wire regressions reproduced these behaviors with an independent aioquic client. These are protocol validation defects, not optional optimizations.

The patch adds validation at the existing decoded-field boundary and rejects malformed trailer fields through the existing stream-error path. It adds no runtime dependency or second parser. Maximus independently enforces body lengths, operation deadlines and reset-on-cancellation. Shared Maximus policy combines split H2/H3 Cookie fields before generic handlers.

The root `Cargo.toml` pins this source through `[patch.crates-io]`. Cargo patches **do not propagate** from a library dependency to its consumer. An application outside this workspace that enables Maximus HTTP/3 must include the same override, adjusted to its checkout:

```toml
[patch.crates-io]
h3 = { path = "path/to/maximus/vendor/h3" }
```

Run `cargo test --manifest-path vendor/h3/Cargo.toml --all-features --locked` plus `scripts/test-interop-linux.sh` when updating this patch. `scripts/check-downstream.sh` verifies the documented external-consumer configuration. Keep the original dependency tests and licenses. Remove the override and vendored copy once a compatible upstream release passes these regressions. Do not publish Maximus HTTP/3 packages without a distributable patched or fixed upstream dependency.

The owned patch can be inspected by comparing these two source files with the crates.io 0.0.8 archive. The source is retained locally because the validation cannot be repaired after h3 has discarded the wire evidence; modifying a developer's global Cargo cache would not produce a reproducible build.
