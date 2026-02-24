mod clock_mock;

use std::sync::atomic::{AtomicU32, Ordering};

use clock_mock::MockClock;
use ratelock::{Clock, RateLimiter};

#[test]
fn init_tokens_equal_capacity() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, 10, &clock);
    assert_eq!(limiter.remaining(), 100);
}

#[test]
fn init_zero_capacity() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(0, 10, &clock);

    assert_eq!(limiter.remaining(), 0);
    assert!(!limiter.allow());

    clock.advance_sec(100);
    assert!(!limiter.allow());
}

#[test]
fn init_zero_refill_rate() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(3, 0, &clock);

    assert!(limiter.allow());
    assert!(limiter.allow());
    assert!(limiter.allow());
    assert!(!limiter.allow());

    clock.advance_sec(1_000);
    assert!(!limiter.allow());
}

#[test]
fn first_allow_succeeds() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(1, 1, &clock);
    assert!(limiter.allow());
}

#[test]
fn exhausted_bucket_denies() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(3, 0, &clock);

    assert!(limiter.allow());
    assert!(limiter.allow());
    assert!(limiter.allow());
    assert!(!limiter.allow());
    assert!(!limiter.allow());
}

#[test]
fn remaining_decreases_on_allow() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(5, 0, &clock);

    assert_eq!(limiter.remaining(), 5);
    let _ = limiter.allow();
    assert_eq!(limiter.remaining(), 4);
    let _ = limiter.allow();
    assert_eq!(limiter.remaining(), 3);
}

#[test]
fn remaining_is_idempotent() {
    let clock = MockClock::new(1_000_000_000);
    let limiter = RateLimiter::with_clock(50, 10, &clock);

    assert!(limiter.allow_n(20));

    let r1 = limiter.remaining();
    let r2 = limiter.remaining();

    assert_eq!(r1, r2);
    assert_eq!(r1, 30);
}

#[test]
fn allow_n_succeeds_within_capacity() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, 0, &clock);

    assert!(limiter.allow_n(50));
    assert_eq!(limiter.remaining(), 50);
}

#[test]
fn allow_n_fails_atomically_no_partial_deduct() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(10, 0, &clock);

    assert!(!limiter.allow_n(11));
    assert_eq!(limiter.remaining(), 10);

    assert!(limiter.allow_n(5));
    assert!(!limiter.allow_n(6));
    assert_eq!(limiter.remaining(), 5);
}

#[test]
fn allow_n_zero_always_succeeds() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(0, 0, &clock);

    assert!(limiter.allow_n(0));
    assert_eq!(limiter.remaining(), 0);
}

#[test]
fn allow_n_exact_capacity() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(50, 0, &clock);

    assert!(limiter.allow_n(50));
    assert_eq!(limiter.remaining(), 0);
    assert!(!limiter.allow());
}

#[test]
fn config_accessors_and_snapshot_are_consistent() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(10, 5, &clock);

    assert_eq!(limiter.capacity(), 10);
    assert_eq!(limiter.refill_per_sec(), 5);
    assert!(limiter.allow_n(3));

    let snapshot_before = limiter.snapshot();
    assert_eq!(snapshot_before.capacity, 10);
    assert_eq!(snapshot_before.refill_per_sec, 5);
    assert_eq!(snapshot_before.tokens, 7);

    clock.advance_sec(1);
    let snapshot_after = limiter.snapshot();
    assert!(snapshot_after.tokens <= 10);
    assert!(snapshot_after.last_refill_ns >= snapshot_before.last_refill_ns);
}

#[cfg(feature = "std")]
#[test]
fn allow_n_concurrent_only_one_wins() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let limiter = Arc::new(RateLimiter::new(100, 0));
    let barrier = Arc::new(Barrier::new(2));
    let successes = Arc::new(std::sync::atomic::AtomicU64::new(0));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let lim = Arc::clone(&limiter);
            let bar = Arc::clone(&barrier);
            let suc = Arc::clone(&successes);
            thread::spawn(move || {
                bar.wait();
                if lim.allow_n(60) {
                    suc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(successes.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(limiter.remaining(), 40);
}

#[cfg(feature = "std")]
#[test]
fn refill_under_contention_stays_bounded() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[derive(Clone)]
    struct ArcClock(Arc<MockClock>);

    impl Clock for ArcClock {
        fn now_ns(&self) -> u64 {
            self.0.now_ns()
        }
    }

    let clock = Arc::new(MockClock::new(0));
    let limiter = Arc::new(RateLimiter::with_clock(
        2_000,
        100_000,
        ArcClock(Arc::clone(&clock)),
    ));

    // Create headroom so refill and consume race on token updates.
    assert!(limiter.allow_n(1_500));
    clock.advance_ms(10);

    let barrier = Arc::new(Barrier::new(12));
    let mut handles = Vec::new();

    // Threads driving refill.
    for _ in 0..4 {
        let lim = Arc::clone(&limiter);
        let clk = Arc::clone(&clock);
        let bar = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            bar.wait();
            for _ in 0..5_000 {
                clk.advance_ns(500);
                let _ = lim.remaining();
            }
        }));
    }

    // Threads consuming tokens concurrently with refill.
    for _ in 0..8 {
        let lim = Arc::clone(&limiter);
        let bar = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            bar.wait();
            for _ in 0..5_000 {
                let _ = lim.allow();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert!(limiter.remaining() <= limiter.capacity());
}

#[test]
fn refill_after_one_second() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, 10, &clock);

    for _ in 0..10 {
        let _ = limiter.allow();
    }

    assert_eq!(limiter.remaining(), 90);

    clock.advance_sec(1);
    let _ = limiter.allow();

    assert_eq!(limiter.remaining(), 99);
}

#[test]
fn refill_capped_at_capacity() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, 1_000, &clock);

    let _ = limiter.allow();
    assert_eq!(limiter.remaining(), 99);

    clock.advance_sec(10);
    let _ = limiter.allow();

    assert_eq!(limiter.remaining(), 99);
}

#[test]
fn partial_refill_sub_second() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, 1_000, &clock);

    for _ in 0..10 {
        let _ = limiter.allow();
    }

    clock.advance_ms(5);
    let _ = limiter.allow();

    assert_eq!(limiter.remaining(), 94);
}

#[test]
fn accumulated_refill_multiple_periods() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, 10, &clock);

    for _ in 0..50 {
        let _ = limiter.allow();
    }

    clock.advance_sec(3);
    let _ = limiter.allow();

    assert_eq!(limiter.remaining(), 79);
}

#[test]
fn burst_up_to_capacity_then_deny() {
    let clock = MockClock::new(0);
    let capacity = 1_000u64;
    let limiter = RateLimiter::with_clock(capacity, 0, &clock);

    let mut successes = 0u64;
    for _ in 0..1_500 {
        if limiter.allow() {
            successes += 1;
        }
    }

    assert_eq!(successes, capacity);
}

#[test]
fn recovery_after_burst() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, 100, &clock);

    for _ in 0..100 {
        let _ = limiter.allow();
    }
    assert!(!limiter.allow());

    clock.advance_sec(1);
    for _ in 0..100 {
        assert!(limiter.allow());
    }
    assert!(!limiter.allow());
}

#[test]
fn custom_clock_trait_impl_works() {
    struct FixedClock(u64);
    impl Clock for FixedClock {
        fn now_ns(&self) -> u64 {
            self.0
        }
    }

    let clock = FixedClock(5_000_000_000);
    let limiter = RateLimiter::with_clock(100, 10, clock);

    assert!(limiter.allow());
    assert_eq!(limiter.remaining(), 99);
}

#[test]
fn clock_called_on_every_allow() {
    struct CountingClock {
        calls: AtomicU32,
        inner: MockClock,
    }

    impl Clock for &CountingClock {
        fn now_ns(&self) -> u64 {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.inner.now_ns()
        }
    }

    let clock = CountingClock {
        calls: AtomicU32::new(0),
        inner: MockClock::new(0),
    };

    let limiter = RateLimiter::with_clock(10, 1, &clock);
    let _ = limiter.allow();
    let _ = limiter.allow();
    let _ = limiter.allow();

    assert!(clock.calls.load(Ordering::Relaxed) >= 3);
}

#[test]
fn scenario_api_rate_limit_100_per_sec() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, 100, &clock);

    let count_allowed = |n: usize| -> u64 { (0..n).filter(|_| limiter.allow()).count() as u64 };

    assert_eq!(count_allowed(150), 100);
    clock.advance_sec(1);
    assert_eq!(count_allowed(150), 100);
}

#[test]
fn scenario_gradual_consumption_matches_refill() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(10, 10, &clock);

    assert!(limiter.allow());

    for step in 0..20 {
        clock.advance_ms(100);
        assert!(limiter.allow(), "failed at step {step}");
    }
}

#[test]
fn scenario_batch_processing_allow_n() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(1_000, 100, &clock);

    for batch in 0..10 {
        assert!(limiter.allow_n(100), "batch {batch} failed");
    }

    assert_eq!(limiter.remaining(), 0);
    assert!(!limiter.allow_n(100));

    clock.advance_sec(1);
    assert!(limiter.allow_n(100));
    assert!(!limiter.allow_n(100));
}
