# Security

Report a suspected vulnerability privately to the repository maintainers through the hosting service's security-reporting channel. Include the affected revision, feature set, request bytes and a minimal reproduction. Do not include private keys or production credentials.

This repository delegates cryptography and HTTP wire parsing to maintained Rust dependencies. Dependency fixes still require protocol interoperability and regression tests. Product unsafe code is confined to `crates/net/src/ebpf.rs`; its borrowed descriptors and pointer lengths must remain valid for the synchronous syscall. Privileged loading examples are test-only.
