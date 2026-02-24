#![cfg(feature = "std")]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

use rate_limit_core::RateLimiter;

#[test]
fn concurrent_allow_no_overdraft() {
    let capacity = 1_000u64;
    let limiter = Arc::new(RateLimiter::new(capacity, 0));
    let barrier = Arc::new(Barrier::new(8));
    let counter = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let lim = Arc::clone(&limiter);
            let bar = Arc::clone(&barrier);
            let cnt = Arc::clone(&counter);
            thread::spawn(move || {
                bar.wait();
                for _ in 0..500 {
                    if lim.allow() {
                        cnt.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let issued = counter.load(Ordering::Relaxed);
    assert!(
        issued <= capacity,
        "overdraft: issued {issued} > capacity {capacity}"
    );
}

#[test]
fn concurrent_allow_exact_count() {
    let capacity = 100u64;
    let threads = 4usize;
    let requests_per_thread = 50usize;

    let limiter = Arc::new(RateLimiter::new(capacity, 0));
    let barrier = Arc::new(Barrier::new(threads));
    let successes = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let lim = Arc::clone(&limiter);
            let bar = Arc::clone(&barrier);
            let suc = Arc::clone(&successes);
            thread::spawn(move || {
                bar.wait();
                let mut local = 0u64;
                for _ in 0..requests_per_thread {
                    if lim.allow() {
                        local += 1;
                    }
                }
                suc.fetch_add(local, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(successes.load(Ordering::Relaxed), capacity);
}

#[test]
fn remaining_never_exceeds_capacity() {
    let capacity = 50u64;
    let limiter = Arc::new(RateLimiter::new(capacity, 0));

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let lim = Arc::clone(&limiter);
            thread::spawn(move || {
                for _ in 0..200 {
                    let r = lim.remaining();
                    assert!(r <= capacity, "remaining {r} > capacity {capacity}");
                    let _ = lim.allow();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn miri_concurrent_safety() {
    let limiter = Arc::new(RateLimiter::new(10, 0));

    let h1 = {
        let l = Arc::clone(&limiter);
        thread::spawn(move || {
            for _ in 0..5 {
                let _ = l.allow();
            }
        })
    };

    let h2 = {
        let l = Arc::clone(&limiter);
        thread::spawn(move || {
            for _ in 0..5 {
                let _ = l.allow();
            }
        })
    };

    h1.join().unwrap();
    h2.join().unwrap();

    assert!(limiter.remaining() <= 10);
}
