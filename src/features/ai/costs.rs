use super::{AiEngine, CostSummary, ModelCost};
use crate::dx::llm_studio::{LlmCostEstimate, LlmStudio};
use std::collections::BTreeMap;

impl AiEngine {
    // 4. Cost Dashboard -----------------------------------------------------
    pub fn log_cost(
        &self,
        model: &str,
        prompt_tokens: usize,
        completion_tokens: usize,
    ) -> LlmCostEstimate {
        let est = LlmStudio::estimate_cost(model, prompt_tokens, completion_tokens);
        self.cost_log.lock().push(est.clone());
        est
    }

    pub fn cost_summary(&self) -> CostSummary {
        let log = self.cost_log.lock();
        let mut per_model: BTreeMap<String, ModelCost> = BTreeMap::new();
        let mut total_cost_usd = 0.0;
        let mut total_tokens = 0;
        for est in log.iter() {
            let mc = per_model.entry(est.model.clone()).or_default();
            mc.calls += 1;
            mc.tokens += est.total_tokens;
            mc.cost_usd += est.total_cost_usd;
            total_cost_usd += est.total_cost_usd;
            total_tokens += est.total_tokens;
        }
        CostSummary {
            per_model,
            total_cost_usd,
            total_tokens,
            call_count: log.len(),
        }
    }

    pub fn reset_costs(&self) {
        self.cost_log.lock().clear();
        self.experiment_costs.lock().clear();
    }

    /// Log a cost and attribute it to an `experiment_id` so spend can be sliced
    /// per A/B arm, prompt revision, or eval run.
    pub fn log_cost_for_experiment(
        &self,
        experiment_id: &str,
        model: &str,
        prompt_tokens: usize,
        completion_tokens: usize,
    ) -> LlmCostEstimate {
        let est = self.log_cost(model, prompt_tokens, completion_tokens);
        let mut map = self.experiment_costs.lock();
        let mc = map.entry(experiment_id.to_string()).or_default();
        mc.calls += 1;
        mc.tokens += est.total_tokens;
        mc.cost_usd += est.total_cost_usd;
        est
    }

    /// Aggregate cost per `experiment_id`.
    pub fn cost_by_experiment(&self) -> BTreeMap<String, ModelCost> {
        self.experiment_costs.lock().clone()
    }
}
