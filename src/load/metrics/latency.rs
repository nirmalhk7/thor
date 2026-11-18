use std::sync::atomic::{AtomicUsize, Ordering};

use super::HISTOGRAM_BUCKETS;

pub(super) fn latency_to_bucket(ms: f64) -> usize {
    if ms <= 0.0 {
        0
    } else if ms < 100.0 {
        (ms as usize).min(99)
    } else if ms < 1000.0 {
        100 + (((ms - 100.0) / 10.0) as usize).min(89)
    } else {
        190 + (((ms - 1000.0) / 100.0) as usize).min(9)
    }
}

pub(super) fn bucket_to_latency(idx: usize) -> f64 {
    if idx < 100 {
        idx as f64
    } else if idx < 190 {
        100.0 + ((idx - 100) as f64 * 10.0)
    } else {
        1000.0 + ((idx - 190) as f64 * 100.0)
    }
}

pub(super) struct LatencySnapshot {
    counts: [usize; HISTOGRAM_BUCKETS],
    total_samples: usize,
    weighted_sum: f64,
    pub(super) min: f64,
    pub(super) max: f64,
}

impl LatencySnapshot {
    pub(super) fn collect(buckets: &[AtomicUsize; HISTOGRAM_BUCKETS]) -> Self {
        let mut snapshot = Self {
            counts: [0; HISTOGRAM_BUCKETS],
            total_samples: 0,
            weighted_sum: 0.0,
            min: f64::MAX,
            max: 0.0,
        };
        for (i, count_slot) in snapshot.counts.iter_mut().enumerate() {
            let count = buckets[i].load(Ordering::Relaxed);
            *count_slot = count;
            if count > 0 {
                let lat = bucket_to_latency(i);
                if lat < snapshot.min {
                    snapshot.min = lat;
                }
                if lat > snapshot.max {
                    snapshot.max = lat;
                }
                snapshot.weighted_sum += lat * (count as f64);
                snapshot.total_samples += count;
            }
        }
        if snapshot.total_samples == 0 {
            snapshot.min = 0.0;
        }
        snapshot
    }

    pub(super) fn avg(&self) -> f64 {
        if self.total_samples > 0 {
            self.weighted_sum / (self.total_samples as f64)
        } else {
            0.0
        }
    }

    pub(super) fn percentile(&self, target_pct: f64) -> f64 {
        if self.total_samples == 0 {
            return 0.0;
        }
        let target_count = ((self.total_samples as f64) * target_pct).round() as usize;
        let mut accum = 0usize;
        for (i, count) in self.counts.iter().enumerate() {
            accum += count;
            if accum >= target_count {
                return bucket_to_latency(i);
            }
        }
        bucket_to_latency(HISTOGRAM_BUCKETS - 1)
    }
}
