use super::{BrowserMetrics, PerformanceBudgets, QaEngine, VitalsGateResult};

impl QaEngine {
    // 6. Real Vitals Gates --------------------------------------------------
    pub fn vitals_gate(
        &self,
        vitals: &BrowserMetrics,
        budgets: &PerformanceBudgets,
    ) -> VitalsGateResult {
        let mut failures = Vec::new();
        let mut gate = |name: &str, actual: f64, budget: Option<f64>| {
            if let Some(b) = budget {
                if actual > b {
                    failures.push(format!("{name} {actual:.1} > budget {b:.1}"));
                }
            }
        };
        gate("LCP", vitals.lcp_ms, budgets.max_lcp_ms);
        gate("INP", vitals.inp_ms, budgets.max_inp_ms);
        gate("CLS", vitals.cls, budgets.max_cls);
        gate("TBT", vitals.tbt_ms, budgets.max_tbt_ms);
        gate("TTFB", vitals.ttfb_ms, budgets.max_ttfb_ms);
        VitalsGateResult {
            passed: failures.is_empty(),
            failures,
        }
    }
}
