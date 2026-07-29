use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Core Web Vitals metrics collected during Browser Runner execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrowserMetrics {
    /// Largest Contentful Paint (ms)
    pub lcp_ms: f64,
    /// Interaction to Next Paint (ms) — modern replacement for FID
    pub inp_ms: f64,
    /// First Input Delay (ms) — legacy compatibility
    pub fid_ms: f64,
    /// Cumulative Layout Shift score
    pub cls: f64,
    /// First Contentful Paint (ms)
    pub fcp_ms: f64,
    /// Time to First Byte (ms)
    pub ttfb_ms: f64,
    /// Total Blocking Time (ms)
    pub tbt_ms: f64,
    /// DOM Content Loaded time (ms)
    pub dom_content_loaded_ms: f64,
    /// DOM Interactive time (ms)
    pub dom_interactive_ms: f64,
}

/// HdrHistogram-style fixed-bucket latency histogram with linear percentile
/// interpolation. Buckets are logarithmically spaced from `min_ms` to `max_ms`
/// so that quantiles remain accurate across several orders of magnitude without
/// retaining every raw sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyHistogram {
    min_ms: f64,
    max_ms: f64,
    /// Upper bound (inclusive) of each bucket in ms.
    bounds: Vec<f64>,
    /// Sample count per bucket.
    counts: Vec<u64>,
    total: u64,
    sum_ms: f64,
    observed_min: f64,
    observed_max: f64,
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        // 0.5ms .. 60_000ms with 3 significant sub-buckets per power of two.
        Self::new(0.5, 60_000.0, 3)
    }
}

impl LatencyHistogram {
    /// Build a histogram spanning `[min_ms, max_ms]` with `sub_buckets` linear
    /// sub-divisions inside each power-of-two band (HdrHistogram semantics).
    pub fn new(min_ms: f64, max_ms: f64, sub_buckets: u32) -> Self {
        let min_ms = min_ms.max(f64::MIN_POSITIVE);
        let max_ms = max_ms.max(min_ms * 2.0);
        let sub = sub_buckets.max(1);
        let mut bounds = Vec::new();
        let mut band = min_ms;
        while band < max_ms {
            let next_band = band * 2.0;
            let step = (next_band - band) / (sub as f64);
            for i in 1..=sub {
                let bound = band + step * (i as f64);
                if bound > bounds.last().copied().unwrap_or(0.0) {
                    bounds.push(bound.min(max_ms));
                }
            }
            band = next_band;
        }
        if bounds.last().copied().unwrap_or(0.0) < max_ms {
            bounds.push(max_ms);
        }
        // Overflow bucket for anything above max_ms.
        bounds.push(f64::INFINITY);
        let counts = vec![0u64; bounds.len()];
        Self {
            min_ms,
            max_ms,
            bounds,
            counts,
            total: 0,
            sum_ms: 0.0,
            observed_min: f64::INFINITY,
            observed_max: 0.0,
        }
    }

    pub fn record(&mut self, value_ms: f64) {
        if value_ms.is_nan() || value_ms < 0.0 {
            return;
        }
        let idx = self.bucket_index(value_ms);
        self.counts[idx] += 1;
        self.total += 1;
        self.sum_ms += value_ms;
        self.observed_min = self.observed_min.min(value_ms);
        self.observed_max = self.observed_max.max(value_ms);
    }

    fn bucket_index(&self, value_ms: f64) -> usize {
        match self.bounds.binary_search_by(|b| {
            b.partial_cmp(&value_ms)
                .unwrap_or(std::cmp::Ordering::Greater)
        }) {
            Ok(i) => i,
            Err(i) => i.min(self.bounds.len() - 1),
        }
    }

    pub fn count(&self) -> u64 {
        self.total
    }

    pub fn mean(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.sum_ms / (self.total as f64)
        }
    }

    pub fn min(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.observed_min
        }
    }

    pub fn max(&self) -> f64 {
        self.observed_max
    }

    /// Percentile with linear interpolation between the bucket's lower and upper
    /// bound based on the fractional rank inside the containing bucket.
    pub fn percentile(&self, q: f64) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let q = q.clamp(0.0, 100.0);
        let target = (q / 100.0) * (self.total as f64);
        let mut cumulative = 0u64;
        let mut lower_bound = 0.0f64;
        for (i, &count) in self.counts.iter().enumerate() {
            if count == 0 {
                lower_bound = self.finite_bound(i);
                continue;
            }
            let prev_cumulative = cumulative;
            cumulative += count;
            if (cumulative as f64) >= target {
                let upper = self.finite_bound(i);
                let within = target - (prev_cumulative as f64);
                let frac = (within / (count as f64)).clamp(0.0, 1.0);
                let interpolated = lower_bound + (upper - lower_bound) * frac;
                return interpolated.clamp(self.observed_min, self.observed_max);
            }
            lower_bound = self.finite_bound(i);
        }
        self.observed_max
    }

    fn finite_bound(&self, i: usize) -> f64 {
        let b = self.bounds[i];
        if b.is_finite() {
            b
        } else {
            self.observed_max.max(self.max_ms)
        }
    }

    pub fn merge(&mut self, other: &LatencyHistogram) {
        if self.bounds == other.bounds {
            for (dst, src) in self.counts.iter_mut().zip(other.counts.iter()) {
                *dst += *src;
            }
            self.total += other.total;
            self.sum_ms += other.sum_ms;
            if other.total > 0 {
                self.observed_min = self.observed_min.min(other.observed_min);
                self.observed_max = self.observed_max.max(other.observed_max);
            }
        }
    }
}

/// Aggregated summary metrics for a load testing run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadMetrics {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub total_bytes: u64,
    pub duration_secs: f64,
    pub rps: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p90_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub status_codes: HashMap<u16, usize>,
    pub browser_vitals: Option<BrowserMetrics>,
}

const HISTOGRAM_BUCKETS: usize = 200;

fn latency_to_bucket(ms: f64) -> usize {
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

fn bucket_to_latency(idx: usize) -> f64 {
    if idx < 100 {
        idx as f64
    } else if idx < 190 {
        100.0 + ((idx - 100) as f64 * 10.0)
    } else {
        1000.0 + ((idx - 190) as f64 * 100.0)
    }
}

/// Thread-safe concurrent metrics collector for load runs (Lock-Free Atomic Histogram)
pub struct MetricsCollector {
    start_time: Instant,
    total_requests: AtomicUsize,
    successful_requests: AtomicUsize,
    failed_requests: AtomicUsize,
    total_bytes: AtomicU64,
    buckets: [AtomicUsize; HISTOGRAM_BUCKETS],
    status_codes: Mutex<HashMap<u16, usize>>,
    browser_vitals: Mutex<Option<BrowserMetrics>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let mut buckets: [AtomicUsize; HISTOGRAM_BUCKETS] = unsafe { std::mem::zeroed() };
        for b in &mut buckets {
            *b = AtomicUsize::new(0);
        }

        Self {
            start_time: Instant::now(),
            total_requests: AtomicUsize::new(0),
            successful_requests: AtomicUsize::new(0),
            failed_requests: AtomicUsize::new(0),
            total_bytes: AtomicU64::new(0),
            buckets,
            status_codes: Mutex::new(HashMap::new()),
            browser_vitals: Mutex::new(None),
        }
    }

    pub fn record_sample(&self, status_code: u16, latency_ms: f64, bytes: u64, is_success: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if is_success {
            self.successful_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
        self.total_bytes.fetch_add(bytes, Ordering::Relaxed);

        let bucket_idx = latency_to_bucket(latency_ms);
        self.buckets[bucket_idx].fetch_add(1, Ordering::Relaxed);

        if let Ok(mut codes) = self.status_codes.lock() {
            *codes.entry(status_code).or_insert(0) += 1;
        }
    }

    pub fn set_browser_vitals(&self, vitals: BrowserMetrics) {
        if let Ok(mut b) = self.browser_vitals.lock() {
            *b = Some(vitals);
        }
    }

    pub fn snapshot(&self) -> LoadMetrics {
        let duration_secs = self.start_time.elapsed().as_secs_f64().max(0.001);
        let total = self.total_requests.load(Ordering::Relaxed);
        let success = self.successful_requests.load(Ordering::Relaxed);
        let failed = self.failed_requests.load(Ordering::Relaxed);
        let bytes = self.total_bytes.load(Ordering::Relaxed);
        let rps = (total as f64) / duration_secs;

        let mut total_samples = 0usize;
        let mut min = f64::MAX;
        let mut max = 0.0f64;
        let mut weighted_sum = 0.0f64;
        let mut counts = [0usize; HISTOGRAM_BUCKETS];

        for i in 0..HISTOGRAM_BUCKETS {
            let count = self.buckets[i].load(Ordering::Relaxed);
            counts[i] = count;
            if count > 0 {
                let lat = bucket_to_latency(i);
                if lat < min {
                    min = lat;
                }
                if lat > max {
                    max = lat;
                }
                weighted_sum += lat * (count as f64);
                total_samples += count;
            }
        }

        if total_samples == 0 {
            min = 0.0;
        }

        let avg = if total_samples > 0 {
            weighted_sum / (total_samples as f64)
        } else {
            0.0
        };

        let calc_p = |target_pct: f64| -> f64 {
            if total_samples == 0 {
                return 0.0;
            }
            let target_count = ((total_samples as f64) * target_pct).round() as usize;
            let mut accum = 0usize;
            for i in 0..HISTOGRAM_BUCKETS {
                accum += counts[i];
                if accum >= target_count {
                    return bucket_to_latency(i);
                }
            }
            bucket_to_latency(HISTOGRAM_BUCKETS - 1)
        };

        let p50 = calc_p(0.50);
        let p90 = calc_p(0.90);
        let p95 = calc_p(0.95);
        let p99 = calc_p(0.99);

        let status_codes = self.status_codes.lock().map(|c| c.clone()).unwrap_or_default();
        let browser_vitals = self.browser_vitals.lock().ok().and_then(|b| b.clone());

        LoadMetrics {
            total_requests: total,
            successful_requests: success,
            failed_requests: failed,
            total_bytes: bytes,
            duration_secs,
            rps,
            min_latency_ms: min,
            max_latency_ms: max,
            avg_latency_ms: avg,
            p50_latency_ms: p50,
            p90_latency_ms: p90,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            status_codes,
            browser_vitals,
        }
    }
}

pub struct LockFreeHistogram {
    pub buckets: [AtomicU64; 200],
}

impl Default for LockFreeHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl LockFreeHistogram {
    pub fn new() -> Self {
        Self {
            buckets: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    pub fn record(&self, latency_ms: f64) {
        let bucket_idx = latency_to_bucket(latency_ms);
        self.buckets[bucket_idx].fetch_add(1, Ordering::Relaxed);
    }

    pub fn calculate_p50_p90_p95_p99(&self) -> (f64, f64, f64, f64) {
        let mut total_samples = 0u64;
        let mut counts = [0u64; 200];
        for i in 0..200 {
            let count = self.buckets[i].load(Ordering::Relaxed);
            counts[i] = count;
            total_samples += count;
        }

        let calc_p = |target_pct: f64| -> f64 {
            if total_samples == 0 {
                return 0.0;
            }
            let target_count = ((total_samples as f64) * target_pct).round() as u64;
            let mut accum = 0u64;
            for i in 0..200 {
                accum += counts[i];
                if accum >= target_count {
                    return bucket_to_latency(i);
                }
            }
            bucket_to_latency(199)
        };

        (calc_p(0.50), calc_p(0.90), calc_p(0.95), calc_p(0.99))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_percentile_interpolation() {
        let mut h = LatencyHistogram::new(1.0, 10_000.0, 4);
        for v in 1..=1000 {
            h.record(v as f64);
        }
        assert_eq!(h.count(), 1000);
        let p50 = h.percentile(50.0);
        let p90 = h.percentile(90.0);
        let p99 = h.percentile(99.0);
        assert!((p50 - 500.0).abs() < 60.0, "p50={p50}");
        assert!((p90 - 900.0).abs() < 80.0, "p90={p90}");
        assert!(p99 > p90 && p99 <= 1000.0, "p99={p99}");
        assert!(p50 < p90);
    }

    #[test]
    fn test_histogram_merge() {
        let mut a = LatencyHistogram::default();
        let mut b = LatencyHistogram::default();
        for _ in 0..100 {
            a.record(50.0);
            b.record(150.0);
        }
        a.merge(&b);
        assert_eq!(a.count(), 200);
        assert!(a.min() <= 50.0 + 1.0);
        assert!(a.max() >= 150.0 - 1.0);
    }
}
