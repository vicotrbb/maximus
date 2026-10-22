"""Socket case contract shared by the runner and qualification reporter."""

# protocol, paths, echo request bytes, generator, frozen executable
SOCKET_CASES = (
    ("plain", "/plaintext,/users/42,/error", 1024, "wrk", "maximus-comparison"),
    ("plain", "/sse", 1024, "k6", "maximus-comparison"),
    ("h1", "/sse", 1024, "k6", "maximus-comparison"),
    ("h2", "/sse", 1024, "k6", "maximus-comparison"),
    ("h2", "/echo", 131072, "k6", "maximus-comparison"),
    ("plain", "/compress", 1024, "k6", "gzip-server"),
    ("plain", "/echo", 1024, "k6", "maximus-comparison"),
    ("plain", "/echo", 131072, "k6", "maximus-comparison"),
    ("h1", "/plaintext,/users/42,/error,/echo", 1024, "k6", "maximus-comparison"),
    ("h2", "/plaintext,/users/42,/error,/echo", 1024, "k6", "maximus-comparison"),
    ("h1", "/echo", 131072, "k6", "maximus-comparison"),
)

DEFAULT_CASES = ",".join(str(case) for case in range(len(SOCKET_CASES)))


def response_keys(case, engines):
    protocol, paths, size, _, _ = SOCKET_CASES[case]
    return {
        (protocol, path, size if path == "/echo" else 0, engine)
        for path in paths.split(",")
        for engine in engines
    }
