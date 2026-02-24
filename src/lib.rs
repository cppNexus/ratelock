#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! A minimal, auditable, lock-free token bucket rate limiter.
//!
//! # Design goals
//! - Zero dependencies
//! - Zero heap allocations
//! - No `unsafe`
//! - `no_std` compatible core
//! - Deterministic tests via custom [`Clock`] implementations
//!
//! # Behavior guarantees
//! - [`RateLimiter::remaining`] is always bounded by `capacity`
//! - [`RateLimiter::allow_n`] is atomic for token deduction
//! - If time goes backwards (`now < last_refill`), refill is skipped
//! - All public operations are panic-free
//!
//! # Example (`std`)
//! ```rust
//! # #[cfg(feature = "std")]
//! # {
//! use rate_limit_core::RateLimiter;
//!
//! let limiter = RateLimiter::new(10, 5);
//! assert!(limiter.allow());
//! assert_eq!(limiter.remaining(), 9);
//! # }
//! ```
//!
//! # Example (`no_std` compatible API)
//! ```rust
//! use rate_limit_core::{Clock, RateLimiter};
//!
//! struct FixedClock(u64);
//! impl Clock for FixedClock {
//!     fn now_ns(&self) -> u64 {
//!         self.0
//!     }
//! }
//!
//! let clock = FixedClock(0);
//! let limiter = RateLimiter::with_clock(3, 0, clock);
//! assert!(limiter.allow());
//! assert!(limiter.allow());
//! assert!(limiter.allow());
//! assert!(!limiter.allow());
//! ```

mod clock;
mod limiter;
mod math;
mod sharded;

pub use clock::Clock;
#[cfg(feature = "std")]
pub use clock::StdClock;
pub use limiter::{RateLimiter, Snapshot};
pub use sharded::ShardedRateLimiter;
