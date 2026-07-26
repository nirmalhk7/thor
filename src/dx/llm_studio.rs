use serde::{Deserialize, Serialize};

/// Cost estimation result for AI model invocations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCostEstimate {
    pub model: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub total_cost_usd: f64,
}

/// Model pricing per 1,000 tokens
#[derive(Debug, Clone)]
pub struct LlmModelPricing {
    pub model_name: &'static str,
    pub input_price_per_1k: f64,
    pub output_price_per_1k: f64,
}

pub const KNOWN_MODELS: &[LlmModelPricing] = &[
    LlmModelPricing {
        model_name: "gpt-4",
        input_price_per_1k: 0.03,
        output_price_per_1k: 0.06,
    },
    LlmModelPricing {
        model_name: "gpt-4o",
        input_price_per_1k: 0.0025,
        output_price_per_1k: 0.01,
    },
    LlmModelPricing {
        model_name: "claude-3.5-sonnet",
        input_price_per_1k: 0.003,
        output_price_per_1k: 0.015,
    },
    LlmModelPricing {
        model_name: "claude-3-opus",
        input_price_per_1k: 0.015,
        output_price_per_1k: 0.075,
    },
    LlmModelPricing {
        model_name: "gemini-1.5-pro",
        input_price_per_1k: 0.00125,
        output_price_per_1k: 0.005,
    },
    LlmModelPricing {
        model_name: "gemini-1.5-flash",
        input_price_per_1k: 0.000075,
        output_price_per_1k: 0.0003,
    },
    LlmModelPricing {
        model_name: "llama-3-70b",
        input_price_per_1k: 0.0007,
        output_price_per_1k: 0.0009,
    },
];

/// Metrics captured during real-time SSE stream parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseStreamMetrics {
    pub ttft_ms: Option<u64>,
    pub total_tokens: usize,
    pub total_chunks: usize,
    pub tokens_per_sec: f64,
    pub accumulated_text: String,
    pub is_complete: bool,
}

/// SSE Stream Parser for tracking TTFT and streaming throughput
#[derive(Debug, Clone)]
pub struct SseStreamParser {
    start_time_ms: u64,
    first_token_time_ms: Option<u64>,
    total_chunks: usize,
    total_tokens: usize,
    accumulated_text: String,
    is_complete: bool,
}

impl SseStreamParser {
    pub fn new(start_time_ms: u64) -> Self {
        Self {
            start_time_ms,
            first_token_time_ms: None,
            total_chunks: 0,
            total_tokens: 0,
            accumulated_text: String::new(),
            is_complete: false,
        }
    }

    /// Process a raw SSE chunk string (e.g. `data: {"choices":[{"delta":{"content":"Hello"}}]}`)
    pub fn feed_chunk(&mut self, chunk_raw: &str, current_time_ms: u64) -> SseStreamMetrics {
        self.total_chunks += 1;

        // Parse content delta from SSE data frame
        for line in chunk_raw.lines() {
            let trimmed = line.trim();
            if trimmed == "data: [DONE]" {
                self.is_complete = true;
                continue;
            }

            if let Some(payload) = trimmed.strip_prefix("data: ") {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(payload) {
                    // OpenAI / generic format check: choices[0].delta.content or choices[0].text
                    let content_delta = val["choices"][0]["delta"]["content"]
                        .as_str()
                        .or_else(|| val["choices"][0]["text"].as_str())
                        .or_else(|| val["delta"]["text"].as_str())
                        .or_else(|| val["content"].as_str());

                    if let Some(text) = content_delta {
                        if !text.is_empty() {
                            if self.first_token_time_ms.is_none() {
                                self.first_token_time_ms = Some(current_time_ms);
                            }
                            let chunk_tokens = LlmStudio::count_tokens(text);
                            self.total_tokens += chunk_tokens;
                            self.accumulated_text.push_str(text);
                        }
                    }
                } else if !payload.is_empty() && payload != "[DONE]" {
                    // Raw string payload
                    if self.first_token_time_ms.is_none() {
                        self.first_token_time_ms = Some(current_time_ms);
                    }
                    let chunk_tokens = LlmStudio::count_tokens(payload);
                    self.total_tokens += chunk_tokens;
                    self.accumulated_text.push_str(payload);
                }
            }
        }

        let ttft_ms = self
            .first_token_time_ms
            .map(|t| t.saturating_sub(self.start_time_ms));

        let duration_after_ttft_sec = self
            .first_token_time_ms
            .map(|t| (current_time_ms.saturating_sub(t) as f64) / 1000.0)
            .unwrap_or(0.0);

        let tokens_per_sec = if duration_after_ttft_sec > 0.001 {
            (self.total_tokens as f64) / duration_after_ttft_sec
        } else {
            0.0
        };

        SseStreamMetrics {
            ttft_ms,
            total_tokens: self.total_tokens,
            total_chunks: self.total_chunks,
            tokens_per_sec,
            accumulated_text: self.accumulated_text.clone(),
            is_complete: self.is_complete,
        }
    }
}

/// AI / LLM Specialist Lens
pub struct LlmStudio;

impl LlmStudio {
    /// BPE heuristic token counter (~4 chars or 0.75 words per token)
    pub fn count_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        let char_count = text.chars().count();
        let word_count = text.split_whitespace().count();

        // Blend character-based and word-based estimation
        let char_estimate = (char_count as f64 / 4.0).ceil() as usize;
        let word_estimate = ((word_count as f64) / 0.75).ceil() as usize;

        char_estimate.max(word_estimate).max(1)
    }

    /// Estimate API cost for given model and token counts
    pub fn estimate_cost(
        model_name: &str,
        prompt_tokens: usize,
        completion_tokens: usize,
    ) -> LlmCostEstimate {
        let name_lower = model_name.to_lowercase();
        let pricing = KNOWN_MODELS
            .iter()
            .find(|m| name_lower == m.model_name)
            .or_else(|| KNOWN_MODELS.iter().find(|m| name_lower.contains(m.model_name)))
            .unwrap_or(&KNOWN_MODELS[0]);

        let input_cost_usd = (prompt_tokens as f64 / 1000.0) * pricing.input_price_per_1k;
        let output_cost_usd = (completion_tokens as f64 / 1000.0) * pricing.output_price_per_1k;
        let total_cost_usd = input_cost_usd + output_cost_usd;

        LlmCostEstimate {
            model: pricing.model_name.to_string(),
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            input_cost_usd,
            output_cost_usd,
            total_cost_usd,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counter() {
        let text = "Hello world, this is a test prompt for Thor AI Lens!";
        let tokens = LlmStudio::count_tokens(text);
        assert!(tokens >= 10 && tokens <= 18);
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

        let chunk2 = "data: {\"choices\":[{\"delta\":{\"content\":\"world!\"}}]}\n\ndata: [DONE]\n\n";
        let metrics2 = parser.feed_chunk(chunk2, 1150);

        assert_eq!(metrics2.accumulated_text, "Hello world!");
        assert!(metrics2.is_complete);
    }
}
