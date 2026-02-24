# rate-limit-core

A minimal, auditable **token bucket** rate limiter for Rust.

`rate-limit-core` is designed as a small core crate you can embed anywhere: API gateways, middleware, embedded services, job schedulers, or any hot path where lock contention and allocations hurt.

Repository: `https://github.com/cppNexus/rate-limit-core.git`

## Problem Statement

Most rate-limit crates are convenient, but frequently include one or more of these trade-offs:
- extra dependencies,
- allocation-heavy internals,
- blocking synchronization primitives,
- tight coupling to async runtimes or framework-specific adapters.

`rate-limit-core` focuses on a small, deterministic kernel that is easy to audit and benchmark.

## Design Goals

- `0` heap allocations in steady-state operations.
- `0` `unsafe` code.
- lock-free internals (`AtomicU64`, no `Mutex`/`RwLock`).
- deterministic refill behavior via pluggable `Clock`.
- `no_std`-compatible core.
- straightforward API: `allow`, `allow_n`, `remaining`, `snapshot`.

## Non-Goals

- no async waiting/sleeping.
- no fairness queueing.
- no distributed rate limiting.
- no leaky-bucket/sliding-window algorithms.
- no framework adapters (Tower/Axum/etc.) in this crate.

## Minimal Example

```rust
use rate_limit_core::RateLimiter;

let limiter = RateLimiter::new(100, 20); // capacity=100, refill=20/sec

if limiter.allow() {
    // process request
} else {
    // reject request
}
```

Batch usage:

```rust
use rate_limit_core::RateLimiter;

let limiter = RateLimiter::new(1000, 100);
assert!(limiter.allow_n(100));
assert_eq!(limiter.remaining(), 900);
```

## API

- `RateLimiter::new(capacity, refill_per_sec)` (`std` feature only)
- `RateLimiter::with_clock(capacity, refill_per_sec, clock)`
- `ShardedRateLimiter::<_, N>::new(capacity, refill_per_sec)` (`std` feature only)
- `ShardedRateLimiter::<_, N>::with_clock(capacity, refill_per_sec, clock)`
- `allow() -> bool`
- `allow_n(n: u64) -> bool`
- `remaining() -> u64`
- `snapshot() -> Snapshot`

## Safety and Correctness Guarantees

- **No panics** in public operations.
- **No allocations** in limiter operations.
- **No unsafe code**.
- `remaining() <= capacity` is always maintained.
- `allow_n` token deduction is atomic: if it returns `false`, tokens are not deducted.
- If a clock returns a timestamp lower than the previous one, refill is skipped (defensive behavior).

## `no_std`

Default features include `std`.

Build without `std`:

```bash
cargo build --no-default-features
```

Use `with_clock` in `no_std` mode by providing your own `Clock` implementation.

## Benchmark Modes

Benchmarks are split into two modes:

| Mode | What it measures | Criterion group |
|------|------------------|-----------------|
| `hot-path` | Shared limiter with `refill_per_sec = 0` (pure atomic contention path) | `single_thread_hot_path/*`, `multi_thread_hot_path/*` |
| `refill-path` | Shared limiter with `refill_per_sec > 0` and manual clock ticks | `single_thread_refill_path/*`, `multi_thread_refill_path/*` |
| `sharded-hot-path` | `N` independent limiters selected by hash (reduced contention) | `multi_thread_sharded_hot_path/*` |
| `governor-compare` | Head-to-head overhead comparison vs `governor` | `single_thread_governor_compare/*`, `multi_thread_governor_compare/*` |

Multi-thread groups are tuned to avoid under-sampling warnings:
- `sample_size(40)`
- `measurement_time(12s)`
- `warm_up_time(3s)`

Run benchmarks:

```bash
cargo bench --bench compare
```

### Benchmarking Guide

1. Run the full benchmark suite:

```bash
cargo bench --bench compare
```

2. Open the Criterion HTML report:

`target/criterion/report/index.html`

3. Run only a specific group or benchmark:

```bash
cargo bench --bench compare -- single_thread_hot_path
cargo bench --bench compare -- multi_thread_hot_path/8
cargo bench --bench compare -- multi_thread_refill_path/8
cargo bench --bench compare -- multi_thread_sharded_hot_path/8
cargo bench --bench compare -- single_thread_governor_compare
cargo bench --bench compare -- multi_thread_governor_compare
```

4. For stable numbers, run the same benchmark 3-5 times and compare medians.

5. If you need a CLI-only run (without plots):

```bash
cargo bench --bench compare -- --noplot
```

## Full Benchmark Table (MacBook Pro M3 Pro, 18GB RAM)

Environment:
- CPU: Apple M3 Pro
- RAM: 18 GB
- OS: macOS 26.3 (build 25D125)
- Date: 2026-02-24

| Benchmark | Time (95% CI) | Throughput (95% CI) |
|---|---:|---:|
| `single_thread_hot_path/allow` | `1.913 - 1.929 ns` | `518.54 - 522.83 M ops/s` |
| `single_thread_hot_path/allow_n_10` | `1.908 - 1.930 ns` | `518.13 - 524.20 M ops/s` |
| `single_thread_hot_path/allow_n_100` | `1.944 - 2.066 ns` | `484.05 - 514.40 M ops/s` |
| `single_thread_refill_path/allow_with_tick` | `13.921 - 14.276 ns` | `70.049 - 71.832 M ops/s` |
| `multi_thread_hot_path/4` | `1.849 - 1.896 ms` | `42.188 - 43.257 M ops/s` |
| `multi_thread_hot_path/8` | `9.190 - 9.819 ms` | `16.295 - 17.411 M ops/s` |
| `multi_thread_hot_path/16` | `36.438 - 38.937 ms` | `8.219 - 8.782 M ops/s` |
| `multi_thread_refill_path/4` | `1.876 - 1.899 ms` | `42.137 - 42.634 M ops/s` |
| `multi_thread_refill_path/8` | `12.973 - 13.862 ms` | `11.542 - 12.333 M ops/s` |
| `multi_thread_refill_path/16` | `46.086 - 49.046 ms` | `6.524 - 6.944 M ops/s` |
| `multi_thread_sharded_hot_path/4` | `788.18 - 793.24 us` | `100.85 - 101.50 M ops/s` |
| `multi_thread_sharded_hot_path/8` | `2.155 - 2.165 ms` | `73.904 - 74.243 M ops/s` |
| `multi_thread_sharded_hot_path/16` | `4.398 - 4.408 ms` | `72.601 - 72.758 M ops/s` |

### rate-limit-core vs governor (same run)

| Scenario | rate-limit-core throughput | governor throughput | Speedup |
|---|---:|---:|---:|
| Single-thread hot check | `524.90 M ops/s` | `233.21 M ops/s` | `2.25x` |
| Shared limiter, 4 threads | `41.99 M ops/s` | `26.75 M ops/s` | `1.57x` |
| Shared limiter, 8 threads | `12.37 M ops/s` | `11.10 M ops/s` | `1.11x` |
| Shared limiter, 16 threads | `5.84 M ops/s` | `3.61 M ops/s` | `1.62x` |

## Benchmark Sanity Notes

- The `~2 ns` number is **hot path without refill**.
- Conditions: `refill_per_sec = 0`, no clock advancement, no refill math, large capacity, fully inlined fast path.
- This is expected to approach the cost of an atomic operation plus branch in optimized builds.
- Benchmark code uses referenced/shared limiter instances and `black_box(black_box(limiter).allow())` to prevent call elision.
- `governor` comparison uses a very high quota (`1_000_000_000/s`) to benchmark check-path overhead rather than throttling behavior.
- Refill-enabled paths are intentionally slower and should be evaluated separately.
- Multi-thread numbers for `multi_thread_hot_path/*` and `multi_thread_refill_path/*` use one shared limiter and include contention.

## Contention and Scaling

- Designed for extreme single-instance hot path.
- Under contention, throughput degrades due to a shared atomic state.
- For per-key/per-tenant limiting, sharding is the recommended pattern.

Built-in sharding helper:

```rust
use rate_limit_core::ShardedRateLimiter;

const SHARDS: usize = 64;
let limiter = ShardedRateLimiter::<_, SHARDS>::new(10_000, 1_000);
let allowed = limiter.allow_by_key(&"tenant:42");
assert!(allowed);
```

## Comparison (Qualitative)

| Property            | rate-limit-core |
|---------------------|-----------------|
| Dependencies        | 0               |
| Allocations         | 0               |
| Unsafe              | 0               |
| no_std core         | Yes             |
| Lock-free           | Yes             |
| Built-in sharding   | Yes             |
| Async waiting       | No              |
| Framework adapters  | No              |

## Reproducible Quality Checks

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --no-default-features
cargo bench --bench compare
```

Optional:

```bash
cargo llvm-cov --all-features --html
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test
```

## License

Apache-2.0, see `LICENSE.md`.
