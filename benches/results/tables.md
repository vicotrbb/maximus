# Measured results

## workloads

Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.

| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| actix | /echo | 1 | plain | 0 | 31,238 | 1.738 | 8.947 | 12.178 | 8.50 | 26.5 | 101.0 | 6.86 | 6.86 | 0 | 0 | 0 | 0 |
| actix | /json | 1 | plain | 0 | 34,324 | 1.592 | 8.731 | 11.964 | 7.52 | 26.3 | 101.2 | 6.32 | 6.53 | 0 | 0 | 0 | 0 |
| actix | /plaintext | 1 | plain | 0 | 29,484 | 1.804 | 10.152 | 15.440 | 8.97 | 26.6 | 100.8 | 5.62 | 6.53 | 0 | 0 | 0 | 0 |
| actix | /users/42 | 1 | plain | 0 | 34,214 | 1.601 | 8.655 | 11.861 | 7.89 | 27.0 | 100.9 | 6.06 | 6.55 | 0 | 0 | 0 | 0 |
| axum | /echo | 1 | plain | 0 | 30,727 | 1.753 | 9.165 | 12.259 | 8.68 | 26.6 | 101.0 | 6.52 | 6.52 | 0 | 0 | 0 | 0 |
| axum | /json | 1 | plain | 0 | 34,294 | 1.593 | 8.671 | 11.837 | 7.73 | 26.9 | 101.2 | 6.50 | 6.50 | 0 | 0 | 0 | 0 |
| axum | /plaintext | 1 | plain | 0 | 26,905 | 1.974 | 11.000 | 15.538 | 9.97 | 26.6 | 100.2 | 6.47 | 6.47 | 0 | 0 | 0 | 0 |
| axum | /users/42 | 1 | plain | 0 | 34,167 | 1.612 | 8.596 | 11.671 | 7.89 | 27.0 | 101.3 | 6.49 | 6.49 | 0 | 0 | 0 | 0 |
| epoll | /echo | 1 | plain | 0 | 30,950 | 1.744 | 9.135 | 11.775 | 12.65 | 39.1 | 100.9 | 7.93 | 7.93 | 0 | 0 | 0 | 0 |
| epoll | /json | 1 | plain | 0 | 34,849 | 1.568 | 8.565 | 12.678 | 10.60 | 37.6 | 101.2 | 7.66 | 7.66 | 0 | 0 | 0 | 0 |
| epoll | /plaintext | 1 | plain | 0 | 29,907 | 1.783 | 10.122 | 14.786 | 12.14 | 37.7 | 100.0 | 7.85 | 7.85 | 0 | 0 | 0 | 0 |
| epoll | /users/42 | 1 | plain | 0 | 32,414 | 1.681 | 8.959 | 12.445 | 11.78 | 37.7 | 100.9 | 7.93 | 7.93 | 0 | 0 | 0 | 0 |
| hyper | /echo | 1 | plain | 0 | 31,184 | 1.723 | 9.089 | 13.092 | 7.91 | 24.2 | 101.1 | 5.67 | 5.67 | 0 | 0 | 0 | 0 |
| hyper | /json | 1 | plain | 0 | 31,561 | 1.706 | 9.460 | 14.679 | 7.98 | 24.6 | 100.7 | 5.57 | 5.57 | 0 | 0 | 0 | 0 |
| hyper | /plaintext | 1 | plain | 0 | 30,529 | 1.770 | 9.598 | 13.357 | 8.31 | 25.3 | 101.1 | 5.62 | 5.62 | 0 | 0 | 0 | 0 |
| hyper | /users/42 | 1 | plain | 0 | 34,005 | 1.591 | 8.771 | 12.752 | 7.23 | 24.6 | 100.6 | 5.59 | 5.59 | 0 | 0 | 0 | 0 |
| maximus | /echo | 1 | plain | 0 | 30,962 | 1.743 | 9.178 | 12.192 | 10.62 | 32.9 | 101.1 | 7.78 | 7.78 | 0 | 0 | 0 | 0 |
| maximus | /json | 1 | plain | 0 | 33,260 | 1.625 | 9.018 | 12.129 | 10.14 | 33.4 | 101.2 | 7.91 | 7.91 | 0 | 0 | 0 | 0 |
| maximus | /plaintext | 1 | plain | 0 | 30,819 | 1.759 | 9.555 | 13.225 | 10.91 | 33.6 | 100.7 | 7.85 | 7.85 | 0 | 0 | 0 | 0 |
| maximus | /users/42 | 1 | plain | 0 | 33,685 | 1.617 | 8.791 | 12.501 | 9.71 | 32.8 | 100.9 | 7.89 | 7.89 | 0 | 0 | 0 | 0 |
| nginx | /json | 1 | plain | 0 | 34,073 | 1.608 | 8.665 | 11.681 | 7.51 | 26.4 | 101.1 | 36.25 | 36.25 | 0 | 0 | 0 | 0 |
| nginx | /plaintext | 1 | plain | 0 | 31,750 | 1.713 | 9.384 | 14.032 | 8.31 | 26.4 | 101.1 | 36.15 | 36.24 | 0 | 0 | 0 | 0 |
| nginx | /users/42 | 1 | plain | 0 | 30,263 | 1.779 | 9.746 | 14.423 | 8.47 | 25.8 | 100.6 | 36.23 | 36.23 | 0 | 0 | 0 | 0 |

## arrival

Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.

| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| actix | /plaintext | 1 | plain | 5000 | 4,893 | 0.642 | 16.068 | 21.301 | 15.75 | 7.7 | 44.4 | 6.70 | 6.70 | 0 | 0 | 3010 | 3 |
| axum | /plaintext | 1 | plain | 5000 | 4,886 | 0.696 | 16.288 | 21.478 | 17.33 | 8.5 | 45.1 | 6.44 | 6.44 | 0 | 0 | 3061 | 3 |
| epoll | /plaintext | 1 | plain | 5000 | 4,908 | 0.604 | 15.773 | 20.030 | 23.65 | 11.6 | 43.0 | 7.68 | 7.68 | 0 | 0 | 2682 | 3 |
| hyper | /plaintext | 1 | plain | 5000 | 4,948 | 0.579 | 13.193 | 17.971 | 14.54 | 7.2 | 40.8 | 5.62 | 5.62 | 0 | 0 | 1557 | 3 |
| maximus | /plaintext | 1 | plain | 5000 | 4,960 | 0.581 | 12.542 | 16.714 | 20.54 | 10.2 | 40.0 | 7.95 | 7.95 | 0 | 0 | 1631 | 3 |

## scaling

Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.

| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| actix | /plaintext | 2 | plain | 0 | 31,568 | 1.711 | 9.482 | 13.219 | 8.87 | 28.0 | 101.0 | 6.83 | 6.83 | 0 | 0 | 0 | 0 |
| axum | /plaintext | 2 | plain | 0 | 33,585 | 1.627 | 8.710 | 12.173 | 8.28 | 27.7 | 101.1 | 6.75 | 6.75 | 0 | 0 | 0 | 0 |
| epoll | /plaintext | 2 | plain | 0 | 32,695 | 1.663 | 8.949 | 13.349 | 12.10 | 39.5 | 100.9 | 8.38 | 8.38 | 0 | 0 | 0 | 0 |
| hyper | /plaintext | 2 | plain | 0 | 31,666 | 1.702 | 9.375 | 12.922 | 8.19 | 25.9 | 101.1 | 5.74 | 5.74 | 0 | 0 | 0 | 0 |
| maximus | /plaintext | 2 | plain | 0 | 32,624 | 1.660 | 9.244 | 13.504 | 10.52 | 34.2 | 101.1 | 8.23 | 8.23 | 0 | 0 | 0 | 0 |

## tls-h1

Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.

| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| axum | /echo | 1 | h1 | 0 | 28,609 | 1.902 | 9.462 | 12.324 | 10.17 | 29.1 | 100.8 | 7.23 | 7.23 | 0 | 0 | 0 | 0 |
| axum | /plaintext | 1 | h1 | 0 | 29,484 | 1.806 | 10.013 | 15.307 | 9.44 | 27.7 | 100.1 | 7.04 | 7.04 | 0 | 0 | 0 | 0 |
| hyper | /echo | 1 | h1 | 0 | 28,378 | 1.916 | 9.498 | 12.533 | 9.68 | 27.5 | 100.7 | 7.23 | 7.23 | 0 | 0 | 0 | 0 |
| hyper | /plaintext | 1 | h1 | 0 | 28,293 | 1.906 | 10.034 | 15.400 | 9.48 | 26.2 | 100.4 | 7.18 | 7.18 | 0 | 0 | 0 | 0 |
| maximus | /echo | 1 | h1 | 0 | 24,980 | 2.105 | 10.610 | 17.143 | 13.75 | 34.3 | 98.9 | 9.04 | 9.04 | 0 | 0 | 0 | 0 |
| maximus | /plaintext | 1 | h1 | 0 | 28,526 | 1.904 | 9.864 | 14.219 | 12.49 | 34.3 | 100.1 | 9.05 | 9.05 | 0 | 0 | 0 | 0 |

## tls-h2

Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.

| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| axum | /echo | 1 | h2 | 0 | 26,315 | 2.062 | 9.728 | 13.891 | 13.95 | 36.6 | 100.0 | 8.79 | 8.79 | 0 | 0 | 0 | 0 |
| axum | /plaintext | 1 | h2 | 0 | 32,460 | 1.715 | 7.526 | 11.430 | 10.73 | 34.8 | 100.9 | 8.81 | 8.81 | 0 | 0 | 0 | 0 |
| hyper | /echo | 1 | h2 | 0 | 26,425 | 2.074 | 9.630 | 12.820 | 13.59 | 35.3 | 100.9 | 8.85 | 8.85 | 0 | 0 | 0 | 0 |
| hyper | /plaintext | 1 | h2 | 0 | 33,180 | 1.678 | 7.498 | 11.360 | 10.22 | 33.7 | 100.9 | 8.84 | 8.84 | 0 | 0 | 0 | 0 |
| maximus | /echo | 1 | h2 | 0 | 26,001 | 2.110 | 9.392 | 12.009 | 17.60 | 45.2 | 100.2 | 10.91 | 10.91 | 0 | 0 | 0 | 0 |
| maximus | /plaintext | 1 | h2 | 0 | 32,024 | 1.707 | 7.696 | 12.186 | 13.36 | 42.8 | 99.4 | 10.98 | 10.98 | 0 | 0 | 0 | 0 |

## sustained

Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.

| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| maximus | /echo | 1 | plain | 0 | 35,628 | 1.668 | 6.293 | 11.376 | 10.76 | 38.3 | 100.7 | 5.07 | 8.03 | 0 | 0 | 0 | 0 |

## capacity

Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.

| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| actix | /plaintext | 1 | plain | 0 | 65,742 | 0.723 | 4.971 | 9.860 | 5.88 | 38.6 | 189.3 | 5.49 | 6.53 | 0 | 0 | 0 | 0 |
| axum | /plaintext | 1 | plain | 0 | 64,416 | 0.742 | 4.999 | 9.713 | 6.26 | 40.3 | 189.4 | 5.74 | 6.27 | 0 | 0 | 0 | 0 |
| epoll | /plaintext | 1 | plain | 0 | 63,035 | 0.753 | 5.156 | 9.620 | 9.29 | 58.6 | 186.9 | 7.85 | 7.85 | 0 | 0 | 0 | 0 |
| hyper | /plaintext | 1 | plain | 0 | 61,923 | 0.768 | 5.266 | 9.790 | 6.21 | 37.8 | 188.6 | 5.51 | 5.51 | 0 | 0 | 0 | 0 |
| maximus | /plaintext | 1 | plain | 0 | 64,336 | 0.735 | 5.104 | 9.772 | 8.38 | 53.9 | 186.4 | 7.57 | 7.57 | 0 | 0 | 0 | 0 |

## arrival-provisioned

Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.

| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| actix | /plaintext | 1 | plain | 5000 | 4,998 | 0.649 | 40.593 | 61.563 | 15.25 | 7.6 | 43.5 | 14.82 | 14.82 | 0 | 0 | 0 | 0 |
| axum | /plaintext | 1 | plain | 5000 | 4,999 | 0.636 | 35.289 | 56.353 | 16.92 | 8.5 | 44.0 | 19.26 | 19.26 | 0 | 0 | 0 | 0 |
| epoll | /plaintext | 1 | plain | 5000 | 4,997 | 0.598 | 28.404 | 45.526 | 23.22 | 11.6 | 41.5 | 31.10 | 31.10 | 0 | 0 | 0 | 0 |
| hyper | /plaintext | 1 | plain | 5000 | 4,998 | 0.661 | 37.635 | 61.293 | 15.45 | 7.7 | 43.5 | 12.56 | 12.56 | 0 | 0 | 0 | 0 |
| maximus | /plaintext | 1 | plain | 5000 | 4,999 | 0.610 | 30.772 | 50.606 | 22.73 | 11.4 | 43.5 | 30.09 | 30.09 | 0 | 0 | 0 | 0 |
