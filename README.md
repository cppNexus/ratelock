# rate-limit-core

A minimal, auditable **token bucket** rate limiter for Rust.

`rate-limit-core` is designed as a small core crate you can embed anywhere: API gateways, middleware, embedded services, job schedulers, or any hot path where lock contention and allocations hurt.

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

## Benchmark Table

Example benchmark groups (run on your hardware):

| Benchmark              | Metric            |
|------------------------|-------------------|
| `single_thread/allow`  | ops/sec           |
| `single_thread/allow_n_10` | ops/sec      |
| `single_thread/allow_n_100` | ops/sec     |
| `multi_thread_allow/4` | ops/sec           |
| `multi_thread_allow/8` | ops/sec           |
| `multi_thread_allow/16`| ops/sec           |

Run benchmarks:

```bash
cargo bench --bench compare
```

## Comparison (Qualitative)

| Property            | rate-limit-core |
|---------------------|-----------------|
| Dependencies        | 0               |
| Allocations         | 0               |
| Unsafe              | 0               |
| no_std core         | Yes             |
| Lock-free           | Yes             |
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
