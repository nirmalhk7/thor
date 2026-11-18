use super::BrowserRunner;
use crate::load::metrics::BrowserMetrics;

impl BrowserRunner {
    /// Derive *synthetic* Core Web Vitals estimates from HTTP latencies.
    ///
    /// **Not lab CWV.** Returns `Err` when inputs are empty (nothing to estimate).
    /// Otherwise returns metrics with [`BrowserMetrics::synthetic`] = true, which
    /// [`PerformanceBudgets`] exclude from gates. Prefer
    /// [`crate::features::cdp_bridge::CdpBridge::parse_web_vitals`] on real
    /// browser runs.
    pub fn derive_vitals(
        nav_latencies: &[f64],
        interaction_latencies: &[f64],
        dom_latency: Option<f64>,
    ) -> Result<BrowserMetrics, String> {
        if nav_latencies.is_empty() && interaction_latencies.is_empty() && dom_latency.is_none() {
            return Err(
                "derive_vitals refused: no latencies to estimate from; collect lab CWV via CDP / PerformanceObserver"
                    .into(),
            );
        }
        let ttfb_ms = percentile(nav_latencies, 50.0).unwrap_or(0.0);
        let fcp_ms = if ttfb_ms > 0.0 { ttfb_ms * 1.35 } else { 0.0 };
        let lcp_ms = nav_latencies
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(fcp_ms);

        let fid_ms = interaction_latencies
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        let fid_ms = if fid_ms.is_finite() { fid_ms } else { 0.0 };
        let inp_ms = percentile(interaction_latencies, 98.0).unwrap_or(fid_ms);
        let tbt_ms: f64 = interaction_latencies
            .iter()
            .map(|&l| (l - 50.0).max(0.0))
            .sum();

        let mut cls = 0.0;
        for w in nav_latencies.windows(2) {
            let base = w[0].max(1.0);
            cls += ((w[1] - w[0]).abs() / base).min(0.1);
        }
        cls = cls.min(1.0);

        let dom = dom_latency.unwrap_or(ttfb_ms);
        Ok(BrowserMetrics {
            lcp_ms,
            fid_ms,
            inp_ms,
            tbt_ms,
            cls,
            fcp_ms,
            ttfb_ms,
            dom_content_loaded_ms: dom * 1.25,
            dom_interactive_ms: dom * 1.1,
            synthetic: true,
        })
    }
}

fn percentile(samples: &[f64], q: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((q / 100.0) * (sorted.len() as f64)).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    Some(sorted[idx])
}
