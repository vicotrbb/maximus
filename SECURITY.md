# Security

Report suspected vulnerabilities privately through [GitHub security advisories](https://github.com/vicotrbb/maximus/security/advisories/new). Include the affected revision, enabled features, request bytes and a minimal reproduction. Do not include private keys or production credentials, or open a public issue with exploit details before coordinated disclosure.

Security fixes target the current `main` branch. There are no published stable releases or supported older release branches yet.

Maximus owns HTTP/1.1 parsing and HTTP/2 framing, stream state and HPACK. Rustls supplies TLS; h3/Quinn supply HTTP/3 and QUIC, with the documented h3 validation patch. Protocol and dependency changes require interoperability and malformed-input regression tests. Product unsafe code is confined to `crates/net/src/ebpf.rs`; its borrowed descriptors and pointer lengths must remain valid for the synchronous syscall. Privileged loading examples are test-only.
