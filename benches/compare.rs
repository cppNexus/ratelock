use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rate_limit_core::RateLimiter;

fn bench_single_thread(c: &mut Criterion) {
    let limiter = RateLimiter::new(1_000_000_000, 0);
    let mut group = c.benchmark_group("single_thread");
    group.throughput(Throughput::Elements(1));

    group.bench_function("allow", |b| {
        b.iter(|| {
            black_box(limiter.allow());
        })
    });

    group.bench_function("allow_n_10", |b| {
        b.iter(|| {
            black_box(limiter.allow_n(10));
        })
    });

    group.bench_function("allow_n_100", |b| {
        b.iter(|| {
            black_box(limiter.allow_n(100));
        })
    });

    group.finish();
}

fn bench_multi_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread_allow");

    for threads in [4usize, 8, 16] {
        group.throughput(Throughput::Elements(threads as u64 * 20_000));
        group.bench_with_input(BenchmarkId::from_parameter(threads), &threads, |b, &t| {
            b.iter(|| {
                let limiter = Arc::new(RateLimiter::new(u64::MAX, 0));
                let success = Arc::new(AtomicU64::new(0));

                let handles: Vec<_> = (0..t)
                    .map(|_| {
                        let lim = Arc::clone(&limiter);
                        let suc = Arc::clone(&success);
                        thread::spawn(move || {
                            let mut local = 0u64;
                            for _ in 0..20_000 {
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

                black_box(success.load(Ordering::Relaxed));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_single_thread, bench_multi_thread);
criterion_main!(benches);
