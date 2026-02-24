# Benchmarks

Full benchmark results for `ratelock`.

Environment: Apple M3 Pro · macOS Tahoe 26.3 · 18 GB RAM · 2026-02-24

## Full Results

| Benchmark | Time (95% CI) | Throughput (95% CI) |
|---|---:|---:|
| `single_thread_hot_path/allow` | `1.913 – 1.929 ns` | `518.54 – 522.83 M ops/s` |
| `single_thread_hot_path/allow_n_10` | `1.908 – 1.930 ns` | `518.13 – 524.20 M ops/s` |
| `single_thread_hot_path/allow_n_100` | `1.944 – 2.066 ns` | `484.05 – 514.40 M ops/s` |
| `single_thread_refill_path/allow_with_tick` | `13.921 – 14.276 ns` | `70.049 – 71.832 M ops/s` |
| `multi_thread_hot_path/4` | `1.849 – 1.896 ms` | `42.188 – 43.257 M ops/s` |
| `multi_thread_hot_path/8` | `9.190 – 9.819 ms` | `16.295 – 17.411 M ops/s` |
| `multi_thread_hot_path/16` | `36.438 – 38.937 ms` | `8.219 – 8.782 M ops/s` |
| `multi_thread_refill_path/4` | `1.876 – 1.899 ms` | `42.137 – 42.634 M ops/s` |
| `multi_thread_refill_path/8` | `12.973 – 13.862 ms` | `11.542 – 12.333 M ops/s` |
| `multi_thread_refill_path/16` | `46.086 – 49.046 ms` | `6.524 – 6.944 M ops/s` |
| `multi_thread_sharded_hot_path/4` | `788.18 – 793.24 µs` | `100.85 – 101.50 M ops/s` |
| `multi_thread_sharded_hot_path/8` | `2.155 – 2.165 ms` | `73.904 – 74.243 M ops/s` |
| `multi_thread_sharded_hot_path/16` | `4.398 – 4.408 ms` | `72.601 – 72.758 M ops/s` |

## ratelock vs governor

> Numbers from the `multi_thread_governor_compare/*` group. `governor` version: `0.10.1`.

| Scenario | ratelock | governor | Speedup |
|---|---:|---:|---:|
| Single-thread hot check | `524.90 M ops/s` | `233.21 M ops/s` | `2.25×` |
| Shared limiter, 4 threads | `41.99 M ops/s` | `26.75 M ops/s` | `1.57×` |
| Shared limiter, 8 threads | `12.37 M ops/s` | `11.10 M ops/s` | `1.11×` |
| Shared limiter, 16 threads | `5.84 M ops/s` | `3.61 M ops/s` | `1.62×` |

To reproduce, run:

```bash
cargo bench --bench compare
```

Then open `target/criterion/report/index.html` for the full Criterion HTML report.