# rate-limit-core

[![Crates.io](https://img.shields.io/crates/v/rate-limit-core.svg)](https://crates.io/crates/rate-limit-core)
[![Docs.rs](https://img.shields.io/docsrs/rate-limit-core)](https://docs.rs/rate-limit-core)
[![Downloads](https://img.shields.io/crates/d/rate-limit-core.svg)](https://crates.io/crates/rate-limit-core)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-success)](https://crates.io/crates/rate-limit-core)
[![no_std](https://img.shields.io/badge/no__std-yes-brightgreen)](https://crates.io/crates/rate-limit-core)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![CI](https://github.com/cppNexus/rate-limit-core/actions/workflows/ci.yml/badge.svg)](https://github.com/cppNexus/rate-limit-core/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/badge/coverage-99%25-brightgreen)](https://github.com/cppNexus/rate-limit-core)

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

## Performance

### Why is it fast?

The steady-state hot path:
- does not call the clock
- does not perform u128 math
- does not allocate
- performs a single atomic state transition

The hot-path approaches the cost of a single atomic decrement in optimized builds on this hardware and compiler configuration.

### Benchmark Summary

Environment: Apple M3 Pro · macOS Tahoe 26.3 · 18 GB RAM · 2026-02-24

| Scenario | Throughput |
|---|---:|
| Single-thread hot check | `~520 M ops/s` |
| Shared limiter, 4 threads | `~43 M ops/s` |
| Shared limiter, 8 threads | `~17 M ops/s` |
| Shared limiter, 16 threads | `~9 M ops/s` |
| Sharded (64 shards), 4 threads | `~101 M ops/s` |
| Sharded (64 shards), 16 threads | `~73 M ops/s` |

Full results: see [`BENCHMARKS.md`](BENCHMARKS.md) or open `target/criterion/report/index.html` after running the suite.

### rate-limit-core vs governor

> Numbers from the `multi_thread_governor_compare/*` benchmark group. `governor` version: `0.10.1`.

| Scenario | rate-limit-core | governor | Speedup |
|---|---:|---:|---:|
| Single-thread hot check | `524.90 M ops/s` | `233.21 M ops/s` | `2.25×` |
| Shared limiter, 4 threads | `41.99 M ops/s` | `26.75 M ops/s` | `1.57×` |
| Shared limiter, 8 threads | `12.37 M ops/s` | `11.10 M ops/s` | `1.11×` |
| Shared limiter, 16 threads | `5.84 M ops/s` | `3.61 M ops/s` | `1.62×` |

## Benchmark Modes

Benchmarks are split into four modes:

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

### Benchmarking Guide

1. Run the full benchmark suite:

```bash
cargo bench --bench compare
```

2. Open the Criterion HTML report:

```
target/criterion/report/index.html
```

3. Run only a specific group or benchmark:

```bash
cargo bench --bench compare -- single_thread_hot_path
cargo bench --bench compare -- multi_thread_hot_path/8
cargo bench --bench compare -- multi_thread_refill_path/8
cargo bench --bench compare -- multi_thread_sharded_hot_path/8
cargo bench --bench compare -- single_thread_governor_compare
cargo bench --bench compare -- multi_thread_governor_compare
```

4. For stable numbers, run the same benchmark 3–5 times and compare medians.

5. CLI-only run (without plots):

```bash
cargo bench --bench compare -- --noplot
```

## Benchmark Sanity Notes

- The `~2 ns` figure is the **hot path without refill**: `refill_per_sec = 0`, no clock advancement, no refill math, large capacity, fully inlined fast path. This approaches the cost of an atomic decrement plus branch in optimized builds.
- Benchmark code uses referenced/shared limiter instances and `black_box(black_box(limiter).allow())` to prevent call elision.
- `governor` comparison uses a very high quota (`1_000_000_000/s`) to benchmark check-path overhead rather than throttling behavior.
- Refill-enabled paths are intentionally slower and should be evaluated separately.
- Multi-thread numbers for `multi_thread_hot_path/*` and `multi_thread_refill_path/*` use one shared limiter and include contention. The `governor-compare` group runs its own thread configuration, so those throughput numbers differ from the raw `multi_thread_hot_path/*` results — this is expected.

## Contention and Scaling

- Designed for extreme single-instance hot path.
- Under contention, throughput degrades due to shared atomic state.
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