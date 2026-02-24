const NANOS_PER_SEC: u128 = 1_000_000_000;

#[inline]
pub(crate) fn refill_tokens(elapsed_ns: u64, refill_per_sec: u64) -> u64 {
    if elapsed_ns == 0 || refill_per_sec == 0 {
        return 0;
    }

    let produced = (elapsed_ns as u128).saturating_mul(refill_per_sec as u128) / NANOS_PER_SEC;
    produced.min(u64::MAX as u128) as u64
}

#[inline]
pub(crate) fn elapsed_for_tokens(tokens: u64, refill_per_sec: u64) -> u64 {
    if tokens == 0 || refill_per_sec == 0 {
        return 0;
    }

    let elapsed = (tokens as u128).saturating_mul(NANOS_PER_SEC) / (refill_per_sec as u128);
    elapsed.min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::{elapsed_for_tokens, refill_tokens};

    #[test]
    fn refill_tokens_uses_floor_math() {
        assert_eq!(refill_tokens(0, 10), 0);
        assert_eq!(refill_tokens(10, 0), 0);
        assert_eq!(refill_tokens(500_000_000, 3), 1);
        assert_eq!(refill_tokens(999_999_999, 1), 0);
        assert_eq!(refill_tokens(1_000_000_000, 1), 1);
    }

    #[test]
    fn elapsed_for_tokens_matches_inverse_floor() {
        assert_eq!(elapsed_for_tokens(1, 10), 100_000_000);
        assert_eq!(elapsed_for_tokens(0, 10), 0);
        assert_eq!(elapsed_for_tokens(1, 0), 0);
    }
}
