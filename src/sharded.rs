use core::array::from_fn;
use core::hash::{Hash, Hasher};

#[cfg(feature = "std")]
use crate::StdClock;
use crate::{Clock, RateLimiter, Snapshot};
#[cfg(feature = "std")]
use core::hash::BuildHasher;

#[repr(align(128))]
struct PaddedShard<T>(T);

/// A fixed-size shard set of independent [`RateLimiter`] instances.
///
/// Sharding reduces contention by routing hashes to independent buckets.
/// Hashes that select the same shard share its quota; this is not per-key or
/// per-tenant isolation. Use a separate limiter per key when isolation is
/// required.
///
/// `N` is the number of shards. `N=0` is allowed and treated as disabled;
/// all `allow*` calls return `false`, and `remaining*` returns `0`.
pub struct ShardedRateLimiter<C: Clock + Clone, const N: usize> {
    shards: [PaddedShard<RateLimiter<C>>; N],
    #[cfg(feature = "std")]
    hash_builder: std::collections::hash_map::RandomState,
}

impl<C: Clock + Clone, const N: usize> ShardedRateLimiter<C, N> {
    /// Creates a sharded limiter with a custom clock.
    ///
    /// Each shard has identical `(capacity, refill_per_sec)` configuration.
    pub fn with_clock(capacity: u64, refill_per_sec: u64, clock: C) -> Self {
        Self {
            shards: from_fn(|_| {
                PaddedShard(RateLimiter::with_clock(
                    capacity,
                    refill_per_sec,
                    clock.clone(),
                ))
            }),
            #[cfg(feature = "std")]
            hash_builder: std::collections::hash_map::RandomState::new(),
        }
    }

    /// Returns the number of shards.
    #[inline]
    pub const fn shard_count(&self) -> usize {
        N
    }

    /// Tries to take one token from the shard selected by `hash`.
    #[inline]
    pub fn allow_by_hash(&self, hash: u64) -> bool {
        self.shard_for_hash(hash).is_some_and(RateLimiter::allow)
    }

    /// Tries to take `n` tokens atomically from the shard selected by `hash`.
    #[inline]
    pub fn allow_n_by_hash(&self, hash: u64, n: u64) -> bool {
        self.shard_for_hash(hash)
            .is_some_and(|limiter| limiter.allow_n(n))
    }

    /// Returns remaining tokens from the shard selected by `hash`.
    #[inline]
    pub fn remaining_by_hash(&self, hash: u64) -> u64 {
        self.shard_for_hash(hash).map_or(0, RateLimiter::remaining)
    }

    /// Returns a snapshot for the shard selected by `hash`.
    #[inline]
    pub fn snapshot_by_hash(&self, hash: u64) -> Option<Snapshot> {
        self.shard_for_hash(hash).map(RateLimiter::snapshot)
    }

    /// Returns a shard by index.
    #[inline]
    pub fn shard(&self, index: usize) -> Option<&RateLimiter<C>> {
        self.shards.get(index).map(|shard| &shard.0)
    }

    /// Returns a shard selected by a precomputed hash.
    #[inline]
    pub fn shard_for_hash(&self, hash: u64) -> Option<&RateLimiter<C>> {
        self.shard(self.index_for_hash(hash)?)
    }

    #[inline]
    fn index_for_hash(&self, hash: u64) -> Option<usize> {
        if N == 0 {
            return None;
        }

        let idx = if N.is_power_of_two() {
            (hash as usize) & (N - 1)
        } else {
            (hash as usize) % N
        };
        Some(idx)
    }

    /// Hashes a key to `u64` using a caller-provided hasher implementation.
    ///
    /// This is separate from the randomized hasher used by the `*_by_key`
    /// methods. Avoid deterministic hashers for attacker-controlled keys when
    /// shard concentration is a concern.
    #[inline]
    pub fn hash_key_with<H: Hasher + Default, K: Hash>(&self, key: &K) -> u64 {
        let mut hasher = H::default();
        key.hash(&mut hasher);
        hasher.finish()
    }

    #[cfg(feature = "std")]
    #[inline]
    fn hash_key<K: Hash>(&self, key: &K) -> u64 {
        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(feature = "std")]
impl<const N: usize> ShardedRateLimiter<StdClock, N> {
    /// Creates a sharded limiter using [`StdClock`].
    #[inline]
    pub fn new(capacity: u64, refill_per_sec: u64) -> Self {
        Self::with_clock(capacity, refill_per_sec, StdClock)
    }

    /// Tries to take one token from the shard selected by `key`.
    ///
    /// Keys that hash to the same shard share its bucket and quota. This helper
    /// does not provide independent per-key limits.
    #[inline]
    pub fn allow_by_key<K: Hash>(&self, key: &K) -> bool {
        self.allow_by_hash(self.hash_key(key))
    }

    /// Tries to take `n` tokens atomically from the shard selected by `key`.
    ///
    /// Keys that hash to the same shard share its bucket and quota. This helper
    /// does not provide independent per-key limits.
    #[inline]
    pub fn allow_n_by_key<K: Hash>(&self, key: &K, n: u64) -> bool {
        self.allow_n_by_hash(self.hash_key(key), n)
    }

    /// Returns the remaining tokens in the shard selected by `key`.
    ///
    /// Keys that hash to the same shard observe the same bucket.
    #[inline]
    pub fn remaining_by_key<K: Hash>(&self, key: &K) -> u64 {
        self.remaining_by_hash(self.hash_key(key))
    }

    /// Returns a snapshot from the shard selected by `key`.
    ///
    /// Keys that hash to the same shard observe the same bucket.
    #[inline]
    pub fn snapshot_by_key<K: Hash>(&self, key: &K) -> Option<Snapshot> {
        self.snapshot_by_hash(self.hash_key(key))
    }
}
