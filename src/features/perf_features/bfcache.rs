use super::{BTreeMap, BfCacheReport, BfCacheSample, PerfEngine};

impl PerfEngine {
    // 16. bfcache Probe -----------------------------------------------------
    pub fn bfcache_probe(&self, samples: &[BfCacheSample]) -> BfCacheReport {
        let n = samples.len();
        if n == 0 {
            return BfCacheReport {
                samples: 0,
                restore_rate: 0.0,
                avg_restore_ms: 0.0,
                top_blockers: Vec::new(),
                rating: "unknown".into(),
            };
        }
        let restored = samples.iter().filter(|s| s.restored).count();
        let restore_rate = restored as f64 / n as f64;
        let restore_times: Vec<f64> = samples
            .iter()
            .filter_map(|s| s.restore_ms.filter(|_| s.restored))
            .collect();
        let avg_restore_ms = if restore_times.is_empty() {
            0.0
        } else {
            restore_times.iter().sum::<f64>() / restore_times.len() as f64
        };
        let mut blockers: BTreeMap<String, usize> = BTreeMap::new();
        for s in samples {
            for r in &s.not_restored_reasons {
                *blockers.entry(r.reason.clone()).or_default() += 1;
            }
        }
        let mut top_blockers: Vec<(String, usize)> = blockers.into_iter().collect();
        top_blockers.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        top_blockers.truncate(5);
        let rating = if restore_rate >= 0.9 {
            "good"
        } else if restore_rate >= 0.5 {
            "needs-improvement"
        } else {
            "poor"
        };
        BfCacheReport {
            samples: n,
            restore_rate,
            avg_restore_ms,
            top_blockers,
            rating: rating.to_string(),
        }
    }
}
