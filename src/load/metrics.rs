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
    /// First Input Delay (ms)
    pub fid_ms: f64,
    /// Interaction to Next Paint (ms)
    pub inp_ms: f64,
    /// Total Blocking Time (ms)
    pub tbt_ms: f64,
    /// Cumulative Layout Shift score
    pub cls: f64,
    /// First Contentful Paint (ms)
    pub fcp_ms: f64,
    /// Time to First Byte (ms)
    pub ttfb_ms: f64,
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
        // Rank position (1-based) targeted by this quantile.
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

    /// Bound of bucket `i`, substituting the observed max for the overflow band.
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

/// Thread-safe concurrent metrics collector for load runs
pub struct MetricsCollector {
    start_time: Instant,
    total_requests: AtomicUsize,
    successful_requests: AtomicUsize,
    failed_requests: AtomicUsize,
    total_bytes: AtomicU64,
    latencies_ms: Mutex<Vec<f64>>,
    histogram: Mutex<LatencyHistogram>,
    status_codes: Mutex<HashMap<u16, usize>>,
    browser_vitals: Mutex<Option<BrowserMetrics>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_requests: AtomicUsize::new(0),
            successful_requests: AtomicUsize::new(0),
            failed_requests: AtomicUsize::new(0),
            total_bytes: AtomicU64::new(0),
            latencies_ms: Mutex::new(Vec::new()),
            histogram: Mutex::new(LatencyHistogram::default()),
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

        if let Ok(mut lats) = self.latencies_ms.lock() {
            lats.push(latency_ms);
        }
        if let Ok(mut hist) = self.histogram.lock() {
            hist.record(latency_ms);
        }

        if let Ok(mut codes) = self.status_codes.lock() {
            *codes.entry(status_code).or_insert(0) += 1;
        }
    }

    /// Snapshot the underlying latency histogram (interpolated percentiles).
    pub fn histogram_snapshot(&self) -> LatencyHistogram {
        self.histogram.lock().map(|h| h.clone()).unwrap_or_default()
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

        let hist = self.histogram_snapshot();
        let (min, max, avg, p50, p90, p95, p99) = if hist.count() == 0 {
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        } else {
            (
                hist.min(),
                hist.max(),
                hist.mean(),
                hist.percentile(50.0),
                hist.percentile(90.0),
                hist.percentile(95.0),
                hist.percentile(99.0),
            )
        };

        let status_codes = self
            .status_codes
            .lock()
            .map(|c| c.clone())
            .unwrap_or_default();
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
