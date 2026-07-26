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

        let mut lats = self.latencies_ms.lock().map(|l| l.clone()).unwrap_or_default();
        lats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let (min, max, avg, p50, p90, p95, p99) = if lats.is_empty() {
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        } else {
            let sum: f64 = lats.iter().sum();
            let avg = sum / (lats.len() as f64);
            let min = lats[0];
            let max = lats[lats.len() - 1];
            let p50 = lats[(lats.len() as f64 * 0.50) as usize % lats.len()];
            let p90 = lats[(lats.len() as f64 * 0.90) as usize % lats.len()];
            let p95 = lats[(lats.len() as f64 * 0.95) as usize % lats.len()];
            let p99 = lats[(lats.len() as f64 * 0.99) as usize % lats.len()];
            (min, max, avg, p50, p90, p95, p99)
        };

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
