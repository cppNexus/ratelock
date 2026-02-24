mod clock_mock;

use clock_mock::MockClock;
use rate_limit_core::ShardedRateLimiter;

#[test]
fn sharded_hash_routes_independent_buckets() {
    let clock = MockClock::new(0);
    let limiter = ShardedRateLimiter::<_, 4>::with_clock(1, 0, &clock);

    assert_eq!(limiter.shard_count(), 4);

    assert!(limiter.allow_by_hash(10));
    assert!(!limiter.allow_by_hash(10));

    assert!(limiter.allow_by_hash(11));
    assert!(!limiter.allow_by_hash(11));
}

#[test]
fn sharded_allow_n_is_atomic_per_shard() {
    let clock = MockClock::new(0);
    let limiter = ShardedRateLimiter::<_, 8>::with_clock(10, 0, &clock);

    assert!(limiter.allow_n_by_hash(42, 6));
    assert!(!limiter.allow_n_by_hash(42, 6));
    assert_eq!(limiter.remaining_by_hash(42), 4);
}

#[test]
fn sharded_snapshot_is_bounded() {
    let clock = MockClock::new(0);
    let limiter = ShardedRateLimiter::<_, 2>::with_clock(5, 100, &clock);

    assert!(limiter.allow_by_hash(1));
    clock.advance_ms(20);

    let snapshot = limiter.snapshot_by_hash(1).expect("snapshot must exist");
    assert!(snapshot.tokens <= snapshot.capacity);
    assert_eq!(snapshot.capacity, 5);
}

#[test]
fn zero_shards_are_safe_and_disabled() {
    let clock = MockClock::new(0);
    let limiter = ShardedRateLimiter::<_, 0>::with_clock(10, 0, &clock);

    assert_eq!(limiter.shard_count(), 0);
    assert!(!limiter.allow_by_hash(1));
    assert!(!limiter.allow_n_by_hash(1, 1));
    assert_eq!(limiter.remaining_by_hash(1), 0);
    assert!(limiter.snapshot_by_hash(1).is_none());
    assert!(limiter.shard_for_hash(1).is_none());
}

#[cfg(feature = "std")]
#[test]
fn std_key_helpers_map_to_same_shard() {
    let limiter = ShardedRateLimiter::<_, 16>::new(2, 0);

    assert!(limiter.allow_by_key(&"tenant:42"));
    assert!(limiter.allow_by_key(&"tenant:42"));
    assert!(!limiter.allow_by_key(&"tenant:42"));

    assert_eq!(limiter.remaining_by_key(&"tenant:42"), 0);
}
