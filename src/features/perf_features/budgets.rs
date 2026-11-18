use super::{
    BrowserMetrics, BudgetEvaluation, BudgetReport, BudgetViolation, LoadMetrics, PerfEngine,
    PerformanceBudgets, ResourceBudgetInputs,
};

impl PerfEngine {
    // 1. Perf Budgets -------------------------------------------------------
    pub fn check_budgets(
        &self,
        budgets: &PerformanceBudgets,
        vitals: Option<&BrowserMetrics>,
        load: Option<&LoadMetrics>,
    ) -> BudgetReport {
        self.check_budgets_with_resources(budgets, vitals, load, None)
    }

    /// Delegates to [`PerformanceBudgets::evaluate`] — single source of truth
    /// shared with [`PerformanceBudgets::check`].
    pub fn check_budgets_with_resources(
        &self,
        budgets: &PerformanceBudgets,
        vitals: Option<&BrowserMetrics>,
        load: Option<&LoadMetrics>,
        resources: Option<&ResourceBudgetInputs>,
    ) -> BudgetReport {
        let (transfer, decoded, lcp_media) = match resources {
            Some(r) => (r.transfer_bytes, r.decoded_bytes, r.lcp_media_bytes),
            None => (None, None, None),
        };
        let BudgetEvaluation {
            passed,
            violations,
            checked,
        } = budgets.evaluate(vitals, load, transfer, decoded, lcp_media);
        BudgetReport {
            passed,
            checked,
            violations: violations
                .into_iter()
                .map(|v| BudgetViolation {
                    metric: v.metric,
                    actual: v.actual,
                    budget: v.budget,
                    over_by: v.over_by,
                })
                .collect(),
        }
    }
}
