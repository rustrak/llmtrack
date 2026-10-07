# llmtrack benchmark

## Chat, 1 concurrent

| target | req/s | p50 ms | p99 ms | added p50 | added p99 | first byte p50 | errors | CPU avg % | mem max MB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| direct | 557 | 1.83 | 2.19 | — | — | — | 0 | 6.3 | 3.9 |
| litellm | 167 | 5.63 | 17.33 | +3.81 | +15.14 | — | 0 | 82.5 | 1845.2 |
| llmtrack | 476 | 2.01 | 5.21 | +0.18 | +3.03 | — | 0 | 32.6 | 38.9 |

## Chat, 16 concurrent

| target | req/s | p50 ms | p99 ms | added p50 | added p99 | first byte p50 | errors | CPU avg % | mem max MB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| direct | 11500 | 1.38 | 1.78 | — | — | — | 0 | 33.0 | 5.6 |
| litellm | 455 | 33.25 | 73.66 | +31.87 | +71.88 | — | 0 | 174.4 | 1919.0 |
| llmtrack | 10716 | 1.48 | 2.07 | +0.11 | +0.29 | — | 0 | 135.8 | 46.4 |

## Chat, 64 concurrent

| target | req/s | p50 ms | p99 ms | added p50 | added p99 | first byte p50 | errors | CPU avg % | mem max MB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| direct | 35573 | 1.79 | 2.75 | — | — | — | 0 | 70.9 | 8.9 |
| litellm | 413 | 153.22 | 429.06 | +151.43 | +426.31 | — | 0 | 177.1 | 1950.7 |
| llmtrack | 13401 | 2.33 | 28.14 | +0.54 | +25.40 | — | 0 | 138.1 | 63.1 |

## Stream, 1 concurrent

| target | req/s | p50 ms | p99 ms | added p50 | added p99 | first byte p50 | errors | CPU avg % | mem max MB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| direct | 529 | 1.94 | 2.66 | — | — | 1.92 | 0 | 9.7 | 9.0 |
| litellm | 87 | 11.18 | 17.92 | +9.25 | +15.26 | 9.63 | 0 | 94.3 | 2005.0 |
| llmtrack | 418 | 2.23 | 3.72 | +0.30 | +1.06 | 1.72 | 0 | 44.8 | 57.2 |

## Stream, 16 concurrent

| target | req/s | p50 ms | p99 ms | added p50 | added p99 | first byte p50 | errors | CPU avg % | mem max MB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| direct | 10985 | 1.46 | 2.02 | — | — | 1.43 | 0 | 51.9 | 9.1 |
| litellm | 165 | 94.08 | 160.25 | +92.62 | +158.23 | 61.95 | 0 | 175.2 | 2029.6 |
| llmtrack | 4374 | 2.56 | 11.81 | +1.11 | +9.79 | 1.70 | 0 | 172.9 | 58.3 |

## Stream, 64 concurrent

| target | req/s | p50 ms | p99 ms | added p50 | added p99 | first byte p50 | errors | CPU avg % | mem max MB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| direct | 28842 | 2.13 | 4.57 | — | — | 1.97 | 0 | 131.1 | 10.6 |
| litellm | 140 | 410.88 | 1019.39 | +408.74 | +1014.82 | 296.45 | 0 | 177.1 | 1860.6 |
| llmtrack | 8185 | 5.38 | 54.40 | +3.24 | +49.83 | 2.68 | 0 | 176.4 | 89.1 |

## Memory after the run

```
bench-llmtrack 76.47MiB / 2GiB
bench-litellm 1.817GiB / 2GiB
```
