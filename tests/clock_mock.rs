use std::sync::atomic::{AtomicU64, Ordering};

use rate_limit_core::Clock;

pub struct MockClock {
    now: AtomicU64,
}

impl MockClock {
    pub const fn new(start_ns: u64) -> Self {
        Self {
            now: AtomicU64::new(start_ns),
        }
    }

    pub fn advance_ns(&self, ns: u64) {
        self.now.fetch_add(ns, Ordering::SeqCst);
    }

    pub fn advance_ms(&self, ms: u64) {
        self.advance_ns(ms.saturating_mul(1_000_000));
    }

    pub fn advance_sec(&self, sec: u64) {
        self.advance_ns(sec.saturating_mul(1_000_000_000));
    }
}

impl Clock for MockClock {
    fn now_ns(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}

impl Clock for &MockClock {
    fn now_ns(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}

#[test]
fn mock_clock_advances_correctly() {
    let clock = MockClock::new(0);
    assert_eq!(clock.now_ns(), 0);

    clock.advance_ms(500);
    assert_eq!(clock.now_ns(), 500_000_000);

    clock.advance_sec(1);
    assert_eq!(clock.now_ns(), 1_500_000_000);
}
