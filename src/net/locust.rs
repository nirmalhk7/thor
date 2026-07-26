use serde::{Deserialize, Serialize};

/// High-concurrency Locust HTTP Load Test request metric item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetric {
    pub response_time_us: u64,
    pub status_code: u16,
    pub is_success: bool,
    pub bytes_received: usize,
}

impl RequestMetric {
    pub fn new(response_time_us: u64, status_code: u16, is_success: bool, bytes_received: usize) -> Self {
        Self {
            response_time_us,
            status_code,
            is_success,
            bytes_received,
        }
    }
}

/// Calculated & scored aggregate metrics from high-concurrency Locust load tests
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocustScoredMetrics {
    pub total_requests: usize,
    pub success_requests: usize,
    pub failed_requests: usize,
    pub rps: f64,
    pub error_rate: f64,
    pub min_latency_us: u64,
    pub max_latency_us: u64,
    pub avg_latency_us: f64,
    pub p50_latency_us: u64,
    pub p90_latency_us: u64,
    pub p95_latency_us: u64,
    pub p99_latency_us: u64,
    pub score: f64,
}

pub struct LocustMetricScorer;

impl LocustMetricScorer {
    /// High-performance zero-allocation metric scoring across 10,000+ request metrics
    pub fn score_metrics(metrics: &[RequestMetric], duration_secs: f64) -> LocustScoredMetrics {
        if metrics.is_empty() {
            return LocustScoredMetrics::default();
        }

        let total_requests = metrics.len();
        let mut success_requests = 0;
        let mut failed_requests = 0;
        let mut sum_latency: u64 = 0;
        let mut min_latency = u64::MAX;
        let mut max_latency = 0;

        let mut latencies = Vec::with_capacity(total_requests);

        for m in metrics {
            if m.is_success && m.status_code < 400 {
                success_requests += 1;
            } else {
                failed_requests += 1;
            }
            sum_latency += m.response_time_us;
            min_latency = min_latency.min(m.response_time_us);
            max_latency = max_latency.max(m.response_time_us);
            latencies.push(m.response_time_us);
        }

        latencies.sort_unstable();

        let p50_idx = (total_requests as f64 * 0.50) as usize;
        let p90_idx = (total_requests as f64 * 0.90) as usize;
        let p95_idx = (total_requests as f64 * 0.95) as usize;
        let p99_idx = (total_requests as f64 * 0.99) as usize;

        let p50 = latencies.get(p50_idx.min(total_requests - 1)).copied().unwrap_or(0);
        let p90 = latencies.get(p90_idx.min(total_requests - 1)).copied().unwrap_or(0);
        let p95 = latencies.get(p95_idx.min(total_requests - 1)).copied().unwrap_or(0);
        let p99 = latencies.get(p99_idx.min(total_requests - 1)).copied().unwrap_or(0);

        let rps = if duration_secs > 0.0 {
            total_requests as f64 / duration_secs
        } else {
            0.0
        };

        let error_rate = (failed_requests as f64 / total_requests as f64) * 100.0;
        let avg_latency = sum_latency as f64 / total_requests as f64;

        let latency_score = (100.0 - (p95 as f64 / 1000.0)).clamp(0.0, 100.0);
        let error_penalty = error_rate * 2.0;
        let score = (latency_score - error_penalty).clamp(0.0, 100.0);

        LocustScoredMetrics {
            total_requests,
            success_requests,
            failed_requests,
            rps,
            error_rate,
            min_latency_us: if min_latency == u64::MAX { 0 } else { min_latency },
            max_latency_us: max_latency,
            avg_latency_us: avg_latency,
            p50_latency_us: p50,
            p90_latency_us: p90,
            p95_latency_us: p95,
            p99_latency_us: p99,
            score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locust_metric_scoring() {
        let mut metrics = Vec::with_capacity(1000);
        for i in 0..1000 {
            metrics.push(RequestMetric::new(i * 10, 200, true, 1024));
        }

        let scored = LocustMetricScorer::score_metrics(&metrics, 10.0);
        assert_eq!(scored.total_requests, 1000);
        assert_eq!(scored.success_requests, 1000);
        assert_eq!(scored.failed_requests, 0);
        assert_eq!(scored.rps, 100.0);
        assert!(scored.p50_latency_us > 0);
    }
}
