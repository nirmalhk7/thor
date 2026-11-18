use super::{PerfEngine, SpeculationLabReport, SpeculationTrial};

impl PerfEngine {
    // 19. Speculation Lab ---------------------------------------------------
    pub fn speculation_lab(&self, trials: &[SpeculationTrial]) -> SpeculationLabReport {
        if trials.is_empty() {
            return SpeculationLabReport {
                trials: 0,
                avg_cold_ms: 0.0,
                avg_prerender_ms: 0.0,
                avg_prefetch_ms: 0.0,
                avg_prerender_savings_ms: 0.0,
                avg_prefetch_savings_ms: 0.0,
                best_url: String::new(),
            };
        }
        let n = trials.len() as f64;
        let avg_cold_ms = trials.iter().map(|t| t.cold_nav_ms).sum::<f64>() / n;
        let prerenders: Vec<(f64, f64)> = trials
            .iter()
            .filter_map(|t| t.prerender_nav_ms.map(|p| (t.cold_nav_ms, p)))
            .collect();
        let prefetches: Vec<(f64, f64)> = trials
            .iter()
            .filter_map(|t| t.prefetch_nav_ms.map(|p| (t.cold_nav_ms, p)))
            .collect();
        let avg_prerender_ms = if prerenders.is_empty() {
            0.0
        } else {
            prerenders.iter().map(|(_, p)| *p).sum::<f64>() / prerenders.len() as f64
        };
        let avg_prefetch_ms = if prefetches.is_empty() {
            0.0
        } else {
            prefetches.iter().map(|(_, p)| *p).sum::<f64>() / prefetches.len() as f64
        };
        let avg_prerender_savings_ms = if prerenders.is_empty() {
            0.0
        } else {
            prerenders
                .iter()
                .map(|(c, p)| (c - p).max(0.0))
                .sum::<f64>()
                / prerenders.len() as f64
        };
        let avg_prefetch_savings_ms = if prefetches.is_empty() {
            0.0
        } else {
            prefetches
                .iter()
                .map(|(c, p)| (c - p).max(0.0))
                .sum::<f64>()
                / prefetches.len() as f64
        };
        let best_url = trials
            .iter()
            .max_by(|a, b| {
                let sa = a
                    .prerender_nav_ms
                    .map(|p| a.cold_nav_ms - p)
                    .or_else(|| a.prefetch_nav_ms.map(|p| a.cold_nav_ms - p))
                    .unwrap_or(0.0);
                let sb = b
                    .prerender_nav_ms
                    .map(|p| b.cold_nav_ms - p)
                    .or_else(|| b.prefetch_nav_ms.map(|p| b.cold_nav_ms - p))
                    .unwrap_or(0.0);
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|t| t.url.clone())
            .unwrap_or_default();
        SpeculationLabReport {
            trials: trials.len(),
            avg_cold_ms,
            avg_prerender_ms,
            avg_prefetch_ms,
            avg_prerender_savings_ms,
            avg_prefetch_savings_ms,
            best_url,
        }
    }
}
