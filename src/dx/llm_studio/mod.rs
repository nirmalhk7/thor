mod stream;

pub use stream::{SseStreamMetrics, SseStreamParser};

use serde::{Deserialize, Serialize};

/// Cost estimation result for AI model invocations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCostEstimate {
    pub model: String,
    pub prompt_tokens: usize,
    pub cached_prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub total_cost_usd: f64,
}

/// Model pricing per 1,000,000 tokens
#[derive(Debug, Clone)]
pub struct LlmModelPricing {
    pub model_name: &'static str,
    pub input_price_per_1m: f64,
    pub cache_read_price_per_1m: f64,
    pub output_price_per_1m: f64,
}

pub const KNOWN_MODELS: &[LlmModelPricing] = &[
    LlmModelPricing {
        model_name: "claude-3.7-sonnet",
        input_price_per_1m: 3.00,
        cache_read_price_per_1m: 0.30,
        output_price_per_1m: 15.00,
    },
    LlmModelPricing {
        model_name: "deepseek-r1",
        input_price_per_1m: 0.55,
        cache_read_price_per_1m: 0.14,
        output_price_per_1m: 2.19,
    },
    LlmModelPricing {
        model_name: "gpt-4o",
        input_price_per_1m: 2.50,
        cache_read_price_per_1m: 1.25,
        output_price_per_1m: 10.00,
    },
    LlmModelPricing {
        model_name: "gemini-2.0-flash",
        input_price_per_1m: 0.10,
        cache_read_price_per_1m: 0.025,
        output_price_per_1m: 0.40,
    },
    LlmModelPricing {
        model_name: "claude-3.5-sonnet",
        input_price_per_1m: 3.00,
        cache_read_price_per_1m: 0.30,
        output_price_per_1m: 15.00,
    },
    LlmModelPricing {
        model_name: "llama-3-70b",
        input_price_per_1m: 0.70,
        cache_read_price_per_1m: 0.70,
        output_price_per_1m: 0.90,
    },
];

/// Local AI Provider status detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLlmEndpoint {
    pub provider: String,
    pub base_url: String,
    pub is_available: bool,
    pub detected_models: Vec<String>,
}

/// AI / LLM Specialist Lens
pub struct LlmStudio;

impl LlmStudio {
    /// Export clean context for AI with secrets redacted
    pub fn clean_context_export(raw_context: &str, secrets: &[&str]) -> String {
        let mut clean = raw_context.to_string();
        for secret in secrets {
            if !secret.is_empty() {
                clean = clean.replace(secret, "[REDACTED]");
            }
        }
        clean
    }

    /// BPE heuristic token counter (~4 chars or 0.75 words per token)
    pub fn count_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        let char_count = if text.is_ascii() {
            text.len()
        } else {
            text.chars().count()
        };
        let word_count = text.split_whitespace().count();

        let char_estimate = (char_count as f64 / 4.0).ceil() as usize;
        let word_estimate = ((word_count as f64) / 0.75).ceil() as usize;

        char_estimate.max(word_estimate).max(1)
    }

    /// Estimate API cost with optional prompt caching discount
    pub fn estimate_cost(
        model_name: &str,
        prompt_tokens: usize,
        completion_tokens: usize,
    ) -> LlmCostEstimate {
        Self::estimate_cost_with_caching(model_name, prompt_tokens, 0, completion_tokens)
    }

    /// Estimate API cost considering cached prompt tokens
    pub fn estimate_cost_with_caching(
        model_name: &str,
        prompt_tokens: usize,
        cached_prompt_tokens: usize,
        completion_tokens: usize,
    ) -> LlmCostEstimate {
        let name_lower = model_name.to_lowercase();
        let pricing = KNOWN_MODELS
            .iter()
            .find(|m| name_lower == m.model_name)
            .or_else(|| {
                KNOWN_MODELS
                    .iter()
                    .find(|m| name_lower.contains(m.model_name))
            })
            .unwrap_or(&KNOWN_MODELS[0]);

        let uncached_prompt = prompt_tokens.saturating_sub(cached_prompt_tokens);
        let input_cost_usd = ((uncached_prompt as f64 / 1_000_000.0) * pricing.input_price_per_1m)
            + ((cached_prompt_tokens as f64 / 1_000_000.0) * pricing.cache_read_price_per_1m);
        let output_cost_usd =
            (completion_tokens as f64 / 1_000_000.0) * pricing.output_price_per_1m;
        let total_cost_usd = input_cost_usd + output_cost_usd;

        LlmCostEstimate {
            model: pricing.model_name.to_string(),
            prompt_tokens,
            cached_prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            input_cost_usd,
            output_cost_usd,
            total_cost_usd,
        }
    }

    /// Probe local Ollama and vLLM developer endpoints
    pub async fn probe_local_llm_endpoints() -> Vec<LocalLlmEndpoint> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(500))
            .build()
            .unwrap_or_default();

        let mut results = Vec::new();

        // 1. Probe Ollama on localhost:11434
        if let Ok(res) = client.get("http://localhost:11434/api/tags").send().await {
            if res.status().is_success() {
                if let Ok(val) = res.json::<serde_json::Value>().await {
                    let models = val["models"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    results.push(LocalLlmEndpoint {
                        provider: "Ollama".to_string(),
                        base_url: "http://localhost:11434".to_string(),
                        is_available: true,
                        detected_models: models,
                    });
                }
            }
        }

        // 2. Probe vLLM / LocalAI on localhost:8000
        if let Ok(res) = client.get("http://localhost:8000/v1/models").send().await {
            if res.status().is_success() {
                results.push(LocalLlmEndpoint {
                    provider: "vLLM".to_string(),
                    base_url: "http://localhost:8000".to_string(),
                    is_available: true,
                    detected_models: vec!["vllm-local".to_string()],
                });
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counter() {
        let text = "Hello world, this is a test prompt for Thor AI Lens!";
        let tokens = LlmStudio::count_tokens(text);
        assert!((10..=18).contains(&tokens));
    }

    #[test]
    fn test_cost_estimator() {
        let est = LlmStudio::estimate_cost("gpt-4o", 1000, 500);
        assert_eq!(est.model, "gpt-4o");
        assert_eq!(est.prompt_tokens, 1000);
        assert_eq!(est.completion_tokens, 500);
        assert!((est.input_cost_usd - 0.0025).abs() < 1e-6);
        assert!((est.output_cost_usd - 0.005).abs() < 1e-6);
    }

    #[test]
    fn test_sse_stream_parser() {
        let mut parser = SseStreamParser::new(1000); // start at 1000ms
        let chunk1 = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\n";
        let metrics1 = parser.feed_chunk(chunk1, 1050); // TTFT should be 50ms

        assert_eq!(metrics1.ttft_ms, Some(50));
        assert_eq!(metrics1.accumulated_text, "Hello ");

        let chunk2 =
            "data: {\"choices\":[{\"delta\":{\"content\":\"world!\"}}]}\n\ndata: [DONE]\n\n";
        let metrics2 = parser.feed_chunk(chunk2, 1150);

        assert_eq!(metrics2.accumulated_text, "Hello world!");
        assert!(metrics2.is_complete);
    }
}
