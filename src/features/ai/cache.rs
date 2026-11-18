use super::{AiEngine, CacheHitEvent, CacheHitLedgerReport, TokenWasteReport, TokenWasteSegment};
use crate::dx::llm_studio::LlmStudio;

impl AiEngine {
    pub fn cache_hit_ledger(&self, events: &[CacheHitEvent]) -> CacheHitLedgerReport {
        let hits = events.iter().filter(|e| e.hit).count();
        let misses = events.len().saturating_sub(hits);
        let tokens_saved: usize = events
            .iter()
            .map(|e| {
                e.cached_tokens.unwrap_or(e.tokens_saved).max(if e.hit {
                    e.tokens_saved
                } else {
                    0
                })
            })
            .sum();
        let hit_rate = if events.is_empty() {
            0.0
        } else {
            hits as f64 / events.len() as f64
        };
        let mut saved_usd = 0.0f64;
        for e in events {
            let model = e.model.as_deref().unwrap_or("gpt-4o");
            let prompt = e.prompt_tokens.unwrap_or(0);
            let cached = e
                .cached_tokens
                .unwrap_or(if e.hit { e.tokens_saved } else { 0 });
            let completion = e.completion_tokens.unwrap_or(0);
            if cached > 0 || e.hit {
                let with_cache = LlmStudio::estimate_cost_with_caching(
                    model,
                    prompt.max(cached),
                    cached,
                    completion,
                );
                let without = LlmStudio::estimate_cost(model, prompt.max(cached), completion);
                saved_usd += (without.total_cost_usd - with_cache.total_cost_usd).max(0.0);
            }
        }
        CacheHitLedgerReport {
            hits,
            misses,
            hit_rate,
            tokens_saved,
            saved_usd,
        }
    }

    /// Parse provider `usage` objects (OpenAI `prompt_tokens_details.cached_tokens`,
    /// Anthropic `cache_read_input_tokens`) into a [`CacheHitEvent`].
    pub fn cache_hit_event_from_usage(
        usage: &serde_json::Value,
        model: &str,
        key: &str,
    ) -> CacheHitEvent {
        let prompt_tokens = usage
            .get("prompt_tokens")
            .or_else(|| usage.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let completion_tokens = usage
            .get("completion_tokens")
            .or_else(|| usage.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let cached = usage
            .pointer("/prompt_tokens_details/cached_tokens")
            .or_else(|| usage.get("cache_read_input_tokens"))
            .or_else(|| usage.get("cached_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        CacheHitEvent {
            key: key.to_string(),
            hit: cached > 0,
            tokens_saved: cached,
            model: Some(model.to_string()),
            prompt_tokens: Some(prompt_tokens),
            cached_tokens: Some(cached),
            completion_tokens: Some(completion_tokens),
        }
    }

    pub fn token_waste_auditor(&self, segments: &[TokenWasteSegment]) -> TokenWasteReport {
        let total: usize = segments.iter().map(|s| s.tokens).sum();
        let unused: Vec<&TokenWasteSegment> =
            segments.iter().filter(|s| !s.cited_in_output).collect();
        let wasted: usize = unused.iter().map(|s| s.tokens).sum();
        let waste_pct = if total == 0 {
            0.0
        } else {
            wasted as f64 / total as f64 * 100.0
        };
        TokenWasteReport {
            total_tokens: total,
            wasted_tokens: wasted,
            waste_pct,
            unused_segments: unused.iter().map(|s| s.label.clone()).collect(),
        }
    }
}
