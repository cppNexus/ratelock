use std::hint::black_box;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use criterion::{
    criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode, Throughput,
};
use governor::clock::DefaultClock;
use governor::state::direct::NotKeyed;
use governor::state::InMemoryState;
use governor::{Quota, RateLimiter as GovernorRateLimiter};
use rate_limit_core::{Clock, RateLimiter, ShardedRateLimiter};

const OPS_PER_THREAD: u64 = 20_000;
const THREADS_SET: [usize; 3] = [4, 8, 16];
const SHARDS: usize = 64;

const REFILL_TICK_EVERY: u64 = 64;
const REFILL_TICK_NS: u64 = 1_000_000;
const GOV_REFILL_PER_SEC: u32 = 1_000_000_000;

type GovLimiter = GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>;

#[derive(Clone)]
struct BenchClock {
    now_ns: Arc<AtomicU64>,
}

impl BenchClock {
    fn new(start_ns: u64) -> Self {
        Self {
            now_ns: Arc::new(AtomicU64::new(start_ns)),
        }
    }

    fn tick_ns(&self, delta_ns: u64) {
        self.now_ns.fetch_add(delta_ns, Ordering::Relaxed);
    }
}

impl Clock for BenchClock {
    fn now_ns(&self) -> u64 {
        self.now_ns.load(Ordering::Relaxed)
    }
}

fn bench_single_thread_hot_path(c: &mut Criterion) {
    let limiter = RateLimiter::new(u64::MAX, 0);
    let limiter = &limiter;
    let mut group = c.benchmark_group("single_thread_hot_path");
    group.throughput(Throughput::Elements(1));

    group.bench_function("allow", |b| {
        b.iter(|| {
            black_box(black_box(limiter).allow());
        })
    });

    group.bench_function("allow_n_10", |b| {
        b.iter(|| {
            black_box(black_box(limiter).allow_n(10));
        })
    });

    group.bench_function("allow_n_100", |b| {
        b.iter(|| {
            black_box(black_box(limiter).allow_n(100));
        })
    });

    group.finish();
}

fn bench_single_thread_refill_path(c: &mut Criterion) {
    let clock = BenchClock::new(0);
    let limiter = RateLimiter::with_clock(100_000, 50_000, clock.clone());
    let limiter = &limiter;
    let mut group = c.benchmark_group("single_thread_refill_path");
    group.throughput(Throughput::Elements(1));

    group.bench_function("allow_with_tick", |b| {
        b.iter(|| {
            clock.tick_ns(REFILL_TICK_NS);
            black_box(black_box(limiter).allow());
        })
    });

    group.finish();
}

fn new_governor_limiter() -> GovLimiter {
    GovernorRateLimiter::direct(Quota::per_second(
        NonZeroU32::new(GOV_REFILL_PER_SEC).expect("governor quota must be non-zero"),
    ))
}

fn bench_single_thread_governor_compare(c: &mut Criterion) {
    let core_limiter = RateLimiter::new(u64::MAX, 0);
    let gov_limiter = new_governor_limiter();
    let core_limiter = &core_limiter;
    let gov_limiter = &gov_limiter;

    let mut group = c.benchmark_group("single_thread_governor_compare");
    group.throughput(Throughput::Elements(1));

    group.bench_function("rate_limit_core_allow_hot", |b| {
        b.iter(|| {
            black_box(black_box(core_limiter).allow());
        })
    });

    group.bench_function("governor_check", |b| {
        b.iter(|| {
            black_box(black_box(gov_limiter).check().is_ok());
        })
    });

    group.finish();
}

fn run_multi_thread_hot_path(threads: usize) -> u64 {
    let limiter = Arc::new(RateLimiter::new(u64::MAX, 0));
    let success = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let lim = Arc::clone(&limiter);
            let suc = Arc::clone(&success);
            thread::spawn(move || {
                let mut local = 0u64;
                for _ in 0..OPS_PER_THREAD {
                    if lim.allow() {
                        local += 1;
                    }
                }
                suc.fetch_add(local, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread join failed");
    }

    success.load(Ordering::Relaxed)
}

#[inline]
fn mix64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^ (x >> 33)
}

fn run_multi_thread_sharded_hot_path(threads: usize) -> u64 {
    let limiter = Arc::new(ShardedRateLimiter::<_, SHARDS>::new(u64::MAX, 0));
    let success = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..threads)
        .map(|thread_id| {
            let lim = Arc::clone(&limiter);
            let suc = Arc::clone(&success);
            thread::spawn(move || {
                let mut local = 0u64;
                for i in 0..OPS_PER_THREAD {
                    let hash = mix64((thread_id as u64) << 32 | i);
                    if lim.allow_by_hash(hash) {
                        local += 1;
                    }
                }
                suc.fetch_add(local, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread join failed");
    }

    success.load(Ordering::Relaxed)
}

fn run_multi_thread_refill_path(threads: usize) -> u64 {
    let clock = BenchClock::new(0);
    let limiter = Arc::new(RateLimiter::with_clock(100_000, 50_000, clock.clone()));
    let success = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let lim = Arc::clone(&limiter);
            let suc = Arc::clone(&success);
            let clk = clock.clone();
            thread::spawn(move || {
                let mut local = 0u64;
                for i in 0..OPS_PER_THREAD {
                    if i % REFILL_TICK_EVERY == 0 {
                        clk.tick_ns(REFILL_TICK_NS);
                    }
                    if lim.allow() {
                        local += 1;
                    }
                }
                suc.fetch_add(local, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread join failed");
    }

    success.load(Ordering::Relaxed)
}

fn run_multi_thread_governor_hot_path(threads: usize) -> u64 {
    let limiter = Arc::new(new_governor_limiter());
    let success = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let lim = Arc::clone(&limiter);
            let suc = Arc::clone(&success);
            thread::spawn(move || {
                let mut local = 0u64;
                for _ in 0..OPS_PER_THREAD {
                    if lim.check().is_ok() {
                        local += 1;
                    }
                }
                suc.fetch_add(local, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread join failed");
    }

    success.load(Ordering::Relaxed)
}

fn bench_multi_thread_hot_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread_hot_path");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(40);
    group.measurement_time(Duration::from_secs(12));
    group.warm_up_time(Duration::from_secs(3));

    for threads in THREADS_SET {
        group.throughput(Throughput::Elements(threads as u64 * OPS_PER_THREAD));
        group.bench_with_input(BenchmarkId::from_parameter(threads), &threads, |b, &t| {
            b.iter(|| {
                black_box(run_multi_thread_hot_path(t));
            });
        });
    }

    group.finish();
}

fn bench_multi_thread_refill_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread_refill_path");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(40);
    group.measurement_time(Duration::from_secs(12));
    group.warm_up_time(Duration::from_secs(3));

    for threads in THREADS_SET {
        group.throughput(Throughput::Elements(threads as u64 * OPS_PER_THREAD));
        group.bench_with_input(BenchmarkId::from_parameter(threads), &threads, |b, &t| {
            b.iter(|| {
                black_box(run_multi_thread_refill_path(t));
            });
        });
    }

    group.finish();
}

fn bench_multi_thread_sharded_hot_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread_sharded_hot_path");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(40);
    group.measurement_time(Duration::from_secs(12));
    group.warm_up_time(Duration::from_secs(3));

    for threads in THREADS_SET {
        group.throughput(Throughput::Elements(threads as u64 * OPS_PER_THREAD));
        group.bench_with_input(BenchmarkId::from_parameter(threads), &threads, |b, &t| {
            b.iter(|| {
                black_box(run_multi_thread_sharded_hot_path(t));
            });
        });
    }

    group.finish();
}

fn bench_multi_thread_governor_compare(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread_governor_compare");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(40);
    group.measurement_time(Duration::from_secs(12));
    group.warm_up_time(Duration::from_secs(3));

    for threads in THREADS_SET {
        group.throughput(Throughput::Elements(threads as u64 * OPS_PER_THREAD));
        group.bench_with_input(
            BenchmarkId::new("rate_limit_core_shared", threads),
            &threads,
            |b, &t| {
                b.iter(|| {
                    black_box(run_multi_thread_hot_path(t));
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("governor_shared", threads),
            &threads,
            |b, &t| {
                b.iter(|| {
                    black_box(run_multi_thread_governor_hot_path(t));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_thread_hot_path,
    bench_single_thread_refill_path,
    bench_single_thread_governor_compare,
    bench_multi_thread_hot_path,
    bench_multi_thread_refill_path,
    bench_multi_thread_sharded_hot_path,
    bench_multi_thread_governor_compare
);
criterion_main!(benches);
