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
        // Initialize array of AtomicUsize
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

        // Read histogram buckets atomically
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

        // Percentile calculation directly from histogram cumulative sums
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
