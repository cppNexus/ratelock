use core::cmp::min;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::math::{elapsed_for_tokens, refill_tokens};
use crate::Clock;
#[cfg(feature = "std")]
use crate::StdClock;

/// Non-allocating lock-free token bucket.
///
/// Tokens are deducted atomically. Refill happens lazily during API calls.
pub struct RateLimiter<C: Clock> {
    capacity: u64,
    refill_per_sec: u64,
    tokens: AtomicU64,
    last_refill_ns: AtomicU64,
    clock: C,
}

/// Best-effort state snapshot for observability.
///
/// Under contention, fields may reflect slightly different moments in time,
/// but `tokens` is always bounded by `capacity`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    /// Current token estimate.
    pub tokens: u64,
    /// Last refill timestamp in nanoseconds.
    pub last_refill_ns: u64,
    /// Configured token bucket capacity.
    pub capacity: u64,
    /// Refill speed in tokens per second.
    pub refill_per_sec: u64,
}

impl<C: Clock> RateLimiter<C> {
    /// Creates a new limiter with a custom clock.
    ///
    /// The bucket starts full (`tokens == capacity`).
    pub fn with_clock(capacity: u64, refill_per_sec: u64, clock: C) -> Self {
        let now = clock.now_ns();
        Self {
            capacity,
            refill_per_sec,
            tokens: AtomicU64::new(capacity),
            last_refill_ns: AtomicU64::new(now),
            clock,
        }
    }

    /// Tries to take one token.
    ///
    /// Returns `true` if a token was consumed.
    ///
    /// # Examples
    /// ```rust
    /// # #[cfg(feature = "std")]
    /// # {
    /// use rate_limit_core::RateLimiter;
    ///
    /// let limiter = RateLimiter::new(10, 5);
    /// assert!(limiter.allow());
    /// assert_eq!(limiter.remaining(), 9);
    /// # }
    /// ```
    #[inline]
    pub fn allow(&self) -> bool {
        self.allow_n(1)
    }

    /// Tries to take `n` tokens atomically.
    ///
    /// If this method returns `false`, token count is not decreased.
    ///
    /// # Examples
    /// ```rust
    /// # #[cfg(feature = "std")]
    /// # {
    /// use rate_limit_core::RateLimiter;
    ///
    /// let limiter = RateLimiter::new(100, 10);
    /// assert!(limiter.allow_n(10));
    /// assert_eq!(limiter.remaining(), 90);
    ///
    /// assert!(!limiter.allow_n(100));
    /// assert_eq!(limiter.remaining(), 90);
    /// # }
    /// ```
    pub fn allow_n(&self, n: u64) -> bool {
        if n == 0 {
            return true;
        }
        if n > self.capacity {
            return false;
        }

        loop {
            self.try_refill(self.clock.now_ns());

            let current = self.tokens.load(Ordering::Acquire);
            if current < n {
                return false;
            }

            let next = current - n;
            if self
                .tokens
                .compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }

            core::hint::spin_loop();
        }
    }

    /// Returns a bounded token estimate.
    ///
    /// Under contention this is a snapshot and may be approximate.
    pub fn remaining(&self) -> u64 {
        self.try_refill(self.clock.now_ns());
        self.bounded_tokens()
    }

    /// Returns configured capacity.
    #[inline]
    pub const fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Returns configured refill rate in tokens per second.
    #[inline]
    pub const fn refill_per_sec(&self) -> u64 {
        self.refill_per_sec
    }

    /// Returns a best-effort snapshot for metrics/debugging.
    pub fn snapshot(&self) -> Snapshot {
        self.try_refill(self.clock.now_ns());

        Snapshot {
            tokens: self.bounded_tokens(),
            last_refill_ns: self.last_refill_ns.load(Ordering::Acquire),
            capacity: self.capacity,
            refill_per_sec: self.refill_per_sec,
        }
    }

    fn bounded_tokens(&self) -> u64 {
        min(self.tokens.load(Ordering::Acquire), self.capacity)
    }

    fn try_refill(&self, now_ns: u64) {
        if self.refill_per_sec == 0 || self.capacity == 0 {
            return;
        }

        loop {
            let last = self.last_refill_ns.load(Ordering::Acquire);
            if now_ns <= last {
                return;
            }

            let elapsed = now_ns - last;
            let add = refill_tokens(elapsed, self.refill_per_sec);
            if add == 0 {
                return;
            }

            let delta_ns = elapsed_for_tokens(add, self.refill_per_sec);
            let new_last = min(last.saturating_add(delta_ns), now_ns);

            match self.last_refill_ns.compare_exchange_weak(
                last,
                new_last,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let _ =
                        self.tokens
                            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                                Some(min(current.saturating_add(add), self.capacity))
                            });
                    return;
                }
                Err(_) => core::hint::spin_loop(),
            }
        }
    }
}

#[cfg(feature = "std")]
impl RateLimiter<StdClock> {
    /// Creates a limiter using [`StdClock`].
    #[inline]
    pub fn new(capacity: u64, refill_per_sec: u64) -> Self {
        Self::with_clock(capacity, refill_per_sec, StdClock)
    }
}
