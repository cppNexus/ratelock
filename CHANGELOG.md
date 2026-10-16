# Changelog

## 0.1.1

- Clarify that shards share quotas when keys map to the same shard.
- Use randomized hashing in the standard-library `*_by_key` helpers.
- Separate shard instances onto 128-byte boundaries to reduce false sharing.
- Round refill timestamps up to avoid advancing them ahead of elapsed time.
- Document conservative refill visibility under thread preemption and the
  non-equivalent `ratelock`/`governor` benchmark paths.
