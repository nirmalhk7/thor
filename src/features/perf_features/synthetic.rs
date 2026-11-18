use super::{LatencyHistogram, PerfEngine, SyntheticCheck, SyntheticSummary};

impl PerfEngine {
    // 5. Synthetic Monitor --------------------------------------------------
    pub fn record_synthetic(&self, check: SyntheticCheck) {
        self.synthetic.lock().push(check);
    }

    pub fn synthetic_summary(&self, target: &str) -> SyntheticSummary {
        let checks: Vec<SyntheticCheck> = self
            .synthetic
            .lock()
            .iter()
            .filter(|c| c.target == target)
            .cloned()
            .collect();
        let count = checks.len();
        if count == 0 {
            return SyntheticSummary {
                target: target.to_string(),
                checks: 0,
                uptime_pct: 0.0,
                avg_latency_ms: 0.0,
                p95_latency_ms: 0.0,
                synthetic_excluded: 0,
            };
        }
        let successes = checks.iter().filter(|c| c.success).count();
        let uptime_pct = successes as f64 / count as f64 * 100.0;
        // Status-only / unmeasured probes never feed latency SLA signal.
        let measured: Vec<&SyntheticCheck> = checks.iter().filter(|c| !c.synthetic).collect();
        let synthetic_excluded = count - measured.len();
        let (avg, p95) = if measured.is_empty() {
            (0.0, 0.0)
        } else {
            let avg = measured.iter().map(|c| c.latency_ms).sum::<f64>() / measured.len() as f64;
            let mut hist = LatencyHistogram::default();
            for c in &measured {
                hist.record(c.latency_ms);
            }
            (avg, hist.percentile(95.0))
        };
        SyntheticSummary {
            target: target.to_string(),
            checks: count,
            uptime_pct,
            avg_latency_ms: avg,
            p95_latency_ms: p95,
            synthetic_excluded,
        }
    }

    /// Seed status-only monitor checks from a recorded session.
    ///
    /// [`NetworkTrace`] has no timing field — refuses invented RTT. Records
    /// `synthetic: true` / `latency_ms: 0` so uptime can still be summarized
    /// while latency avg/p95 stay excluded from SLA signal.
    pub fn synthetic_from_session(
        &self,
        session: &crate::dx::test_generator::RecordedSession,
    ) -> usize {
        let mut count = 0;
        for (i, trace) in session.traces.iter().enumerate() {
            self.record_synthetic(SyntheticCheck {
                target: trace.url.clone(),
                success: trace.status > 0 && trace.status < 400,
                latency_ms: 0.0,
                timestamp_ms: i as u64,
                synthetic: true,
            });
            count += 1;
        }
        count
    }
}
