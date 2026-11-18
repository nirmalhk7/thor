use super::{
    AiEngine, LlmLoadReport, LlmStreamStat, StreamChunk, StreamWaterfall, StreamingSloInput,
    StreamingSloReport,
};
use crate::dx::llm_studio::SseStreamParser;

impl AiEngine {
    // 2. Streaming Waterfall ------------------------------------------------
    pub fn streaming_waterfall(&self, chunks: &[StreamChunk]) -> StreamWaterfall {
        if chunks.is_empty() {
            return StreamWaterfall {
                ttft_ms: 0,
                total_tokens: 0,
                tokens_per_sec: 0.0,
                inter_token_gaps_ms: Vec::new(),
                max_gap_ms: 0,
                total_duration_ms: 0,
            };
        }
        let start = chunks[0].arrival_ms;
        let mut parser = SseStreamParser::new(start);
        let mut token_arrivals: Vec<u64> = Vec::new();
        let mut prev_tokens = 0;
        for c in chunks {
            let m = parser.feed_chunk(&c.raw, c.arrival_ms);
            // Register an arrival timestamp for each newly produced token.
            for _ in prev_tokens..m.total_tokens {
                token_arrivals.push(c.arrival_ms);
            }
            prev_tokens = m.total_tokens;
        }
        let final_metrics = parser.feed_chunk("", chunks.last().unwrap().arrival_ms);
        let ttft_ms = final_metrics.ttft_ms.unwrap_or(0);
        let mut gaps = Vec::new();
        for w in token_arrivals.windows(2) {
            gaps.push(w[1].saturating_sub(w[0]));
        }
        let max_gap_ms = gaps.iter().copied().max().unwrap_or(0);
        let total_duration_ms = chunks.last().unwrap().arrival_ms.saturating_sub(start);
        StreamWaterfall {
            ttft_ms,
            total_tokens: final_metrics.total_tokens,
            tokens_per_sec: final_metrics.tokens_per_sec,
            inter_token_gaps_ms: gaps,
            max_gap_ms,
            total_duration_ms,
        }
    }

    // 11. LLM Load ----------------------------------------------------------
    pub fn llm_load_report(&self, stats: &[LlmStreamStat]) -> LlmLoadReport {
        if stats.is_empty() {
            return LlmLoadReport {
                streams: 0,
                total_tokens: 0,
                aggregate_tokens_per_sec: 0.0,
                avg_ttft_ms: 0.0,
                p95_ttft_ms: 0.0,
            };
        }
        let total_tokens: usize = stats.iter().map(|s| s.tokens).sum();
        let max_duration_ms = stats
            .iter()
            .map(|s| s.duration_ms)
            .max()
            .unwrap_or(0)
            .max(1);
        let aggregate_tokens_per_sec = total_tokens as f64 / (max_duration_ms as f64 / 1000.0);
        let avg_ttft_ms = stats.iter().map(|s| s.ttft_ms as f64).sum::<f64>() / stats.len() as f64;
        let mut ttfts: Vec<u64> = stats.iter().map(|s| s.ttft_ms).collect();
        ttfts.sort_unstable();
        let idx = (((ttfts.len() as f64) * 0.95).ceil() as usize)
            .saturating_sub(1)
            .min(ttfts.len() - 1);
        LlmLoadReport {
            streams: stats.len(),
            total_tokens,
            aggregate_tokens_per_sec,
            avg_ttft_ms,
            p95_ttft_ms: ttfts[idx] as f64,
        }
    }

    pub fn streaming_slo_gates(&self, input: &StreamingSloInput) -> StreamingSloReport {
        let mut violations = Vec::new();
        if input.ttft_ms > input.ttft_budget_ms {
            violations.push(format!(
                "TTFT {}ms > budget {}ms",
                input.ttft_ms, input.ttft_budget_ms
            ));
        }
        if input.tokens_per_sec < input.min_tokens_per_sec {
            violations.push(format!(
                "throughput {:.1} t/s < min {:.1}",
                input.tokens_per_sec, input.min_tokens_per_sec
            ));
        }
        if input.max_gap_ms > input.max_gap_budget_ms {
            violations.push(format!(
                "max gap {}ms > budget {}ms",
                input.max_gap_ms, input.max_gap_budget_ms
            ));
        }
        StreamingSloReport {
            passed: violations.is_empty(),
            violations,
        }
    }

    /// Derive streaming SLO input from a captured [`StreamWaterfall`].
    pub fn streaming_slo_gates_from_waterfall(
        &self,
        waterfall: &StreamWaterfall,
        ttft_budget_ms: u64,
        min_tokens_per_sec: f64,
        max_gap_budget_ms: u64,
    ) -> StreamingSloReport {
        self.streaming_slo_gates(&StreamingSloInput {
            ttft_ms: waterfall.ttft_ms,
            tokens_per_sec: waterfall.tokens_per_sec,
            max_gap_ms: waterfall.max_gap_ms,
            ttft_budget_ms,
            min_tokens_per_sec,
            max_gap_budget_ms,
        })
    }
}
