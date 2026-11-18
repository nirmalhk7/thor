use super::{
    BrowserMetrics, InpReport, InteractionEvent, LatencyHistogram, PerfEngine, RumLabDivergence,
    SoftNavRecord, SoftNavVitalsReport,
};

impl PerfEngine {
    // 6. RUM vs Lab ---------------------------------------------------------
    pub fn rum_vs_lab(&self, lab: &BrowserMetrics, rum: &BrowserMetrics) -> Vec<RumLabDivergence> {
        let pairs = [
            ("lcp_ms", lab.lcp_ms, rum.lcp_ms),
            ("inp_ms", lab.inp_ms, rum.inp_ms),
            ("cls", lab.cls, rum.cls),
            ("ttfb_ms", lab.ttfb_ms, rum.ttfb_ms),
            ("fcp_ms", lab.fcp_ms, rum.fcp_ms),
        ];
        pairs
            .into_iter()
            .map(|(metric, l, r)| RumLabDivergence {
                metric: metric.to_string(),
                lab: l,
                rum: r,
                divergence_pct: if l.abs() > f64::EPSILON {
                    (r - l) / l * 100.0
                } else {
                    0.0
                },
            })
            .collect()
    }

    // 7. HDR Histogram Helper ----------------------------------------------
    pub fn histogram_from_samples(&self, samples: &[f64]) -> LatencyHistogram {
        let mut hist = LatencyHistogram::default();
        for s in samples {
            hist.record(*s);
        }
        hist
    }

    pub fn percentile_table(&self, hist: &LatencyHistogram) -> Vec<(f64, f64)> {
        [50.0, 75.0, 90.0, 95.0, 99.0, 99.9]
            .into_iter()
            .map(|q| (q, hist.percentile(q)))
            .collect()
    }

    // 8. INP Attribution ----------------------------------------------------
    pub fn attribute_inp(&self, interactions: &[InteractionEvent]) -> InpReport {
        if interactions.is_empty() {
            return InpReport {
                inp_ms: 0.0,
                worst_target: String::new(),
                dominant_phase: "none".into(),
                rating: "good".into(),
            };
        }
        // INP ≈ the worst (near-max) interaction latency.
        let worst = interactions
            .iter()
            .max_by(|a, b| {
                a.total_ms()
                    .partial_cmp(&b.total_ms())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        let inp_ms = worst.total_ms();
        let (dominant_phase, _) = [
            ("input-delay", worst.input_delay_ms),
            ("processing", worst.processing_ms),
            ("presentation", worst.presentation_ms),
        ]
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();
        let rating = if inp_ms <= 200.0 {
            "good"
        } else if inp_ms <= 500.0 {
            "needs-improvement"
        } else {
            "poor"
        };
        InpReport {
            inp_ms,
            worst_target: worst.target.clone(),
            dominant_phase: dominant_phase.to_string(),
            rating: rating.to_string(),
        }
    }

    // 10. Vitals Under Load -------------------------------------------------
    /// Find the concurrency "knee" where LCP first crosses the budget as load
    /// increases. Samples are `(concurrency, lcp_ms)`.
    pub fn vitals_knee(&self, samples: &[(u32, f64)], lcp_budget_ms: f64) -> Option<u32> {
        let mut sorted: Vec<(u32, f64)> = samples.to_vec();
        sorted.sort_by_key(|(c, _)| *c);
        sorted
            .into_iter()
            .find(|(_, lcp)| *lcp > lcp_budget_ms)
            .map(|(c, _)| c)
    }

    // 18. Soft-Nav Vitals ---------------------------------------------------
    /// Aggregate soft-navigation CWV records.
    ///
    /// Refuses all-zero theater: when every record has `lcp_ms = inp_ms = cls = 0`,
    /// returns `Err` instead of a fake-good report. Empty input is allowed (no
    /// soft-navs observed) and yields an empty report.
    pub fn soft_nav_vitals(
        &self,
        records: &[SoftNavRecord],
    ) -> Result<SoftNavVitalsReport, String> {
        if !records.is_empty()
            && records
                .iter()
                .all(|r| r.lcp_ms == 0.0 && r.inp_ms == 0.0 && r.cls == 0.0)
        {
            return Err(
                "soft-nav vitals refused: all records have lcp=inp=cls=0 (no real SoftNav CWV)"
                    .into(),
            );
        }
        if records.is_empty() {
            return Ok(SoftNavVitalsReport {
                navigations: 0,
                records: Vec::new(),
                avg_lcp_ms: 0.0,
                worst_lcp_ms: 0.0,
                avg_inp_ms: 0.0,
                total_cls: 0.0,
            });
        }
        let n = records.len() as f64;
        let avg_lcp_ms = records.iter().map(|r| r.lcp_ms).sum::<f64>() / n;
        let worst_lcp_ms = records.iter().map(|r| r.lcp_ms).fold(0.0_f64, f64::max);
        let avg_inp_ms = records.iter().map(|r| r.inp_ms).sum::<f64>() / n;
        let total_cls = records.iter().map(|r| r.cls).sum::<f64>();
        Ok(SoftNavVitalsReport {
            navigations: records.len(),
            records: records.to_vec(),
            avg_lcp_ms,
            worst_lcp_ms,
            avg_inp_ms,
            total_cls,
        })
    }
}
