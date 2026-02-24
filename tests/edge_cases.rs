mod clock_mock;

use clock_mock::MockClock;
use rate_limit_core::RateLimiter;

#[test]
fn max_capacity_no_panic() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(u64::MAX, 1, &clock);

    assert!(limiter.allow());
    assert_eq!(limiter.remaining(), u64::MAX - 1);
}

#[test]
fn high_refill_rate_no_overflow() {
    let clock = MockClock::new(0);
    let limiter = RateLimiter::with_clock(100, u64::MAX / 2, &clock);

    for _ in 0..100 {
        let _ = limiter.allow();
    }

    clock.advance_sec(1);
    let _ = limiter.allow();

    assert!(limiter.remaining() <= 100);
}

#[test]
fn no_time_advance_no_refill() {
    let clock = MockClock::new(1_000_000_000);
    let limiter = RateLimiter::with_clock(100, 10, &clock);

    for _ in 0..10 {
        let _ = limiter.allow();
    }

    let before = limiter.remaining();
    let _ = limiter.allow();
    let after = limiter.remaining();

    assert_eq!(before - 1, after);
}

#[test]
fn time_goes_backward_no_refill_no_panic() {
    let old_clock = MockClock::new(5_000_000_000);
    let limiter = RateLimiter::with_clock(100, 10, &old_clock);

    for _ in 0..10 {
        let _ = limiter.allow();
    }

    old_clock.advance_sec(1);
    assert!(limiter.remaining() <= 100);

    let past_clock = MockClock::new(1_000_000_000);
    let limiter2 = RateLimiter::with_clock(100, 10, &past_clock);

    for _ in 0..10 {
        let _ = limiter2.allow();
    }

    let _ = limiter2.allow();
    assert!(limiter2.remaining() <= 100);
}
