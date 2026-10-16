# Linux hardware results

## wrk

Performance values are medians of independent trials; error/drop/failed-trial counts are totals. Percentiles are medians of per-trial percentiles. wrk uses its corrected histogram; k6 uses request-duration observations. CPU values are approximate; RSS is observed before/after each trial, not a sampled peak.

| Generator | Engine | Path | Workers | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Server CPU % | Client CPU % | RSS MiB | HTTP/check/wrk-network errors | Dropped | Failed trials |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| wrk | actix | /plaintext | 1 | 125,659 | 0.984 | 1.396 | 2.093 | 5.48 | 68.9 | 113.9 | 8.61 | 0/0/0 | 0 | 0 |
| wrk | actix | /plaintext | 2 | 237,294 | 0.525 | 0.798 | 1.953 | 5.88 | 139.4 | 210.6 | 8.62 | 0/0/0 | 0 | 0 |
| wrk | actix | /plaintext | 4 | 407,854 | 0.185 | 2.687 | 5.644 | 6.81 | 279.3 | 290.1 | 8.59 | 0/0/0 | 0 | 0 |
| wrk | axum | /plaintext | 1 | 119,601 | 1.058 | 1.575 | 2.581 | 5.97 | 71.5 | 105.1 | 8.38 | 0/0/0 | 0 | 0 |
| wrk | axum | /plaintext | 2 | 216,568 | 0.605 | 0.971 | 1.400 | 6.69 | 143.9 | 197.3 | 9.25 | 0/0/0 | 0 | 0 |
| wrk | axum | /plaintext | 4 | 377,458 | 0.315 | 0.722 | 4.139 | 7.54 | 284.6 | 287.1 | 9.41 | 0/0/0 | 0 | 0 |
| wrk | epoll | /plaintext | 1 | 70,952 | 1.744 | 2.807 | 4.419 | 11.14 | 78.8 | 65.0 | 11.12 | 0/0/0 | 0 | 0 |
| wrk | epoll | /plaintext | 2 | 136,822 | 0.929 | 1.501 | 2.240 | 11.51 | 157.2 | 130.2 | 11.12 | 0/0/0 | 0 | 0 |
| wrk | epoll | /plaintext | 4 | 248,597 | 0.491 | 0.910 | 2.219 | 12.84 | 319.2 | 226.9 | 11.75 | 0/0/0 | 0 | 0 |
| wrk | hyper | /plaintext | 1 | 133,103 | 0.956 | 1.280 | 1.960 | 5.22 | 69.3 | 116.8 | 6.88 | 0/0/0 | 0 | 0 |
| wrk | hyper | /plaintext | 2 | 241,977 | 0.536 | 0.874 | 1.239 | 5.73 | 139.0 | 214.1 | 7.27 | 0/0/0 | 0 | 0 |
| wrk | hyper | /plaintext | 4 | 405,184 | 0.273 | 0.623 | 3.620 | 6.72 | 272.2 | 291.4 | 7.57 | 0/0/0 | 0 | 0 |
| wrk | maximus | /plaintext | 1 | 119,334 | 1.033 | 1.551 | 2.590 | 6.29 | 75.1 | 79.9 | 11.00 | 0/0/0 | 0 | 0 |
| wrk | maximus | /plaintext | 2 | 213,511 | 0.575 | 0.991 | 3.401 | 7.00 | 149.4 | 166.6 | 11.12 | 0/0/0 | 0 | 0 |
| wrk | maximus | /plaintext | 4 | 354,015 | 0.330 | 0.704 | 3.770 | 8.67 | 306.9 | 277.7 | 11.25 | 0/0/0 | 0 | 0 |

Task-attached PMU counters include attributed kernel/interrupt execution; they are a different scope from cgroup CPU. See the report for the diagnostic. Each trial attaches to existing server TIDs after warmup.

| Engine | Workers | Cycles/request | Instructions/request | Cache misses/request |
| --- | ---: | ---: | ---: | ---: |
| actix | 1 | 31,546 | 24,939 | 337.1 |
| actix | 2 | 31,208 | 24,695 | 328.3 |
| actix | 4 | 31,659 | 25,129 | 343.9 |
| axum | 1 | 33,398 | 27,853 | 346.7 |
| axum | 2 | 34,136 | 27,726 | 401.8 |
| axum | 4 | 34,408 | 27,610 | 441.1 |
| epoll | 1 | 56,278 | 46,249 | 1,133.0 |
| epoll | 2 | 53,971 | 45,674 | 1,073.9 |
| epoll | 4 | 54,349 | 44,731 | 1,110.6 |
| hyper | 1 | 29,865 | 23,468 | 258.6 |
| hyper | 2 | 30,228 | 23,297 | 278.7 |
| hyper | 4 | 31,369 | 23,363 | 358.8 |
| maximus | 1 | 33,993 | 35,471 | 441.1 |
| maximus | 2 | 34,680 | 35,452 | 492.8 |
| maximus | 4 | 37,871 | 37,026 | 663.8 |

## k6

Performance values are medians of independent trials; error/drop/failed-trial counts are totals. Percentiles are medians of per-trial percentiles. wrk uses its corrected histogram; k6 uses request-duration observations. CPU values are approximate; RSS is observed before/after each trial, not a sampled peak.

| Generator | Engine | Path | Workers | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Server CPU % | Client CPU % | RSS MiB | HTTP/check/wrk-network errors | Dropped | Failed trials |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| k6 | actix | /echo | 1 | 43,464 | 1.670 | 14.724 | 23.075 | 10.51 | 45.2 | 368.3 | 9.87 | 0/0/0 | 0 | 0 |
| k6 | actix | /plaintext | 1 | 48,689 | 1.537 | 13.047 | 23.678 | 8.69 | 42.1 | 369.8 | 8.65 | 0/0/0 | 0 | 0 |
| k6 | axum | /echo | 1 | 43,335 | 1.661 | 14.385 | 22.846 | 10.72 | 46.6 | 369.7 | 7.35 | 0/0/0 | 0 | 0 |
| k6 | axum | /plaintext | 1 | 48,233 | 1.525 | 12.952 | 21.705 | 9.46 | 46.4 | 368.9 | 8.74 | 0/0/0 | 0 | 0 |
| k6 | epoll | /echo | 1 | 40,728 | 2.233 | 14.838 | 22.542 | 18.12 | 73.6 | 364.3 | 11.75 | 0/0/0 | 0 | 0 |
| k6 | epoll | /plaintext | 1 | 47,254 | 1.681 | 13.273 | 21.392 | 15.11 | 70.9 | 370.3 | 11.25 | 0/0/0 | 0 | 0 |
| k6 | hyper | /echo | 1 | 43,646 | 1.654 | 14.623 | 23.736 | 9.43 | 40.6 | 371.4 | 7.50 | 0/0/0 | 0 | 0 |
| k6 | hyper | /plaintext | 1 | 47,198 | 1.556 | 13.193 | 22.444 | 8.46 | 39.9 | 367.7 | 7.00 | 0/0/0 | 0 | 0 |
| k6 | maximus | /echo | 1 | 42,477 | 1.736 | 14.319 | 22.833 | 15.16 | 64.4 | 370.1 | 11.25 | 0/0/0 | 0 | 0 |
| k6 | maximus | /plaintext | 1 | 46,710 | 1.598 | 13.613 | 22.703 | 13.52 | 63.2 | 367.0 | 11.12 | 0/0/0 | 0 | 0 |

## arrival

Performance values are medians of independent trials; error/drop/failed-trial counts are totals. Percentiles are medians of per-trial percentiles. wrk uses its corrected histogram; k6 uses request-duration observations. CPU values are approximate; RSS is observed before/after each trial, not a sampled peak.

| Generator | Engine | Path | Workers | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Server CPU % | Client CPU % | RSS MiB | HTTP/check/wrk-network errors | Dropped | Failed trials |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| k6 | actix | /plaintext | 1 | 4,999 | 0.068 | 0.343 | 1.678 | 10.02 | 5.0 | 54.1 | 8.61 | 0/0/0 | 0 | 0 |
| k6 | axum | /plaintext | 1 | 4,999 | 0.072 | 0.332 | 3.015 | 11.28 | 5.6 | 53.9 | 8.75 | 0/0/0 | 0 | 0 |
| k6 | epoll | /plaintext | 1 | 5,000 | 0.096 | 0.339 | 1.851 | 19.77 | 9.9 | 55.5 | 11.62 | 0/0/0 | 0 | 0 |
| k6 | hyper | /plaintext | 1 | 5,000 | 0.068 | 0.396 | 2.074 | 10.00 | 5.0 | 54.3 | 6.88 | 0/0/0 | 0 | 0 |
| k6 | maximus | /plaintext | 1 | 5,000 | 0.090 | 0.377 | 2.024 | 15.93 | 8.0 | 55.4 | 10.75 | 0/0/0 | 0 | 0 |

## sustained

Performance values are medians of independent trials; error/drop/failed-trial counts are totals. Percentiles are medians of per-trial percentiles. wrk uses its corrected histogram; k6 uses request-duration observations. CPU values are approximate; RSS is observed before/after each trial, not a sampled peak.

| Generator | Engine | Path | Workers | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Server CPU % | Client CPU % | RSS MiB | HTTP/check/wrk-network errors | Dropped | Failed trials |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| wrk | maximus | /plaintext | 1 | 114,282 | 1.058 | 2.133 | 3.194 | 6.58 | 75.2 | 78.9 | 11.12 | 0/0/0 | 0 | 0 |

Task-attached PMU counters include attributed kernel/interrupt execution; they are a different scope from cgroup CPU. See the report for the diagnostic. Each trial attaches to existing server TIDs after warmup.

| Engine | Workers | Cycles/request | Instructions/request | Cache misses/request |
| --- | ---: | ---: | ---: | ---: |
| maximus | 1 | 34,802 | 35,757 | 446.7 |
