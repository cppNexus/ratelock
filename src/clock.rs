/// Monotonic time source used by [`crate::RateLimiter`].
///
/// Implementors must guarantee monotonic behavior. If an implementation
/// returns a smaller value than before, refill is intentionally disabled
/// until time catches up.
pub trait Clock {
    /// Returns current monotonic timestamp in nanoseconds.
    fn now_ns(&self) -> u64;
}

#[cfg(feature = "std")]
use std::sync::OnceLock;
#[cfg(feature = "std")]
use std::time::Instant;

#[cfg(feature = "std")]
fn start_instant() -> &'static Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now)
}

/// `std` clock implementation backed by [`std::time::Instant`].
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, Default)]
pub struct StdClock;

#[cfg(feature = "std")]
impl Clock for StdClock {
    fn now_ns(&self) -> u64 {
        let nanos = start_instant().elapsed().as_nanos();
        nanos.min(u64::MAX as u128) as u64
    }
}
