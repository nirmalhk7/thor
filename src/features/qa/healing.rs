use super::{now_ms, ElementSignals, HealPatch, HealSignal, QaEngine, RankedLocator};

impl QaEngine {
    /// Re-rank locators from live heal signals and emit a heal-patch artifact.
    pub fn runtime_self_heal(&self, signals: &ElementSignals, live: &[HealSignal]) -> HealPatch {
        let mut ranked = self.rank_locators(signals);
        for loc in &mut ranked {
            if let Some(sig) = live.iter().find(|s| s.strategy == loc.strategy) {
                let total = sig.successes + sig.failures;
                let success_rate = if total == 0 {
                    0.5
                } else {
                    sig.successes as f64 / total as f64
                };
                let boost = (success_rate * 40.0) as u32;
                let penalty = if sig.failures > sig.successes { 25 } else { 0 };
                loc.resilience_score = loc
                    .resilience_score
                    .saturating_add(boost)
                    .saturating_sub(penalty);
            }
        }
        ranked.sort_by_key(|item| std::cmp::Reverse(item.resilience_score));
        let preferred = ranked.first().cloned().unwrap_or(RankedLocator {
            expression: signals
                .css
                .clone()
                .map(|c| format!("page.locator('{c}')"))
                .unwrap_or_else(|| "page.locator('body')".into()),
            strategy: "fallback".into(),
            resilience_score: 1,
        });
        let patch_json = serde_json::to_string_pretty(&serde_json::json!({
            "preferred": preferred,
            "ranked": ranked,
            "generated_ms": now_ms(),
        }))
        .unwrap_or_else(|_| "{}".into());
        HealPatch {
            preferred,
            ranked,
            patch_json,
        }
    }
}
