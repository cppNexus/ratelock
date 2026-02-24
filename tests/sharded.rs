mod clock_mock;

#[cfg(feature = "std")]
use std::collections::hash_map::DefaultHasher;

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

#[test]
fn non_power_of_two_shards_use_modulo_distribution() {
    let clock = MockClock::new(0);
    let limiter = ShardedRateLimiter::<_, 3>::with_clock(1, 0, &clock);

    assert!(limiter.allow_by_hash(4)); // 4 % 3 == 1
    assert!(!limiter.allow_by_hash(1)); // same shard as 4
}

#[cfg(feature = "std")]
#[test]
fn std_key_helpers_cover_allow_n_and_snapshot() {
    let limiter = ShardedRateLimiter::<_, 8>::new(10, 1_000);
    let key = "tenant:alpha";

    assert!(limiter.allow_n_by_key(&key, 4));
    let snap = limiter
        .snapshot_by_key(&key)
        .expect("snapshot_by_key must return a shard snapshot");
    assert_eq!(snap.capacity, 10);
    assert_eq!(limiter.remaining_by_key(&key), 6);

    let stable_hash = limiter.hash_key_with::<DefaultHasher, _>(&key);
    assert_eq!(
        limiter.remaining_by_hash(stable_hash),
        limiter.remaining_by_key(&key)
    );
}
