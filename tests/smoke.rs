#![cfg(feature = "std")]

use rate_limit_core::RateLimiter;

#[test]
fn smoke_std_new_works() {
    let limiter = RateLimiter::new(100, 10);

    assert_eq!(limiter.remaining(), 100);
    assert!(limiter.allow());
    assert_eq!(limiter.remaining(), 99);
}

#[test]
fn smoke_std_no_panic_under_real_time() {
    let limiter = RateLimiter::new(10, 100);

    for _ in 0..100 {
        let _ = limiter.allow();
    }

    assert!(limiter.remaining() <= 10);
}
