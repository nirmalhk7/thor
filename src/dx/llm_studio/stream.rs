use super::LlmStudio;
use serde::{Deserialize, Serialize};

/// Metrics captured during real-time SSE stream parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseStreamMetrics {
    pub ttft_ms: Option<u64>,
    pub total_tokens: usize,
    pub total_chunks: usize,
    pub tokens_per_sec: f64,
    pub accumulated_text: String,
    pub is_complete: bool,
    pub tool_calls: Vec<String>,
    pub finish_reason: Option<String>,
    pub error: Option<String>,
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
    buffer: String,
    tool_calls: Vec<String>,
    finish_reason: Option<String>,
    error: Option<String>,
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
            buffer: String::new(),
            tool_calls: Vec::new(),
            finish_reason: None,
            error: None,
        }
    }

    /// Process a raw SSE chunk string across OpenAI, Anthropic, Gemini, and Ollama formats
    pub fn feed_chunk(&mut self, chunk_raw: &str, current_time_ms: u64) -> SseStreamMetrics {
        self.total_chunks += 1;
        self.buffer.push_str(chunk_raw);
        while let Some(pos) = self.buffer.find('\n') {
            let line = self.buffer[..pos].to_string();
            self.buffer.drain(..=pos);
            self.process_line(line.trim(), current_time_ms);
        }
        self.metrics(current_time_ms)
    }

    fn process_line(&mut self, trimmed: &str, current_time_ms: u64) {
        if trimmed == "data: [DONE]" || trimmed.contains("\"type\":\"message_stop\"") {
            self.is_complete = true;
            return;
        }
        let payload_str = trimmed.strip_prefix("data: ").unwrap_or(trimmed);
        match serde_json::from_str::<serde_json::Value>(payload_str) {
            Ok(val) => self.process_event(&val, current_time_ms),
            Err(_)
                if !payload_str.is_empty()
                    && payload_str != "[DONE]"
                    && payload_str.starts_with("data:") =>
            {
                self.append_text(payload_str, current_time_ms);
            }
            Err(_) => {}
        }
    }

    fn process_event(&mut self, val: &serde_json::Value, current_time_ms: u64) {
        if let Some(text) = text_delta(val).filter(|text| !text.is_empty()) {
            self.append_text(text, current_time_ms);
        }
        if let Some(calls) = val["choices"][0]["delta"]["tool_calls"].as_array() {
            self.tool_calls.extend(
                calls
                    .iter()
                    .filter_map(|call| call["function"]["name"].as_str().map(str::to_string)),
            );
        }
        if let Some(reason) = val["choices"][0]["finish_reason"].as_str() {
            self.finish_reason = Some(reason.to_string());
            self.is_complete = true;
        }
        if let Some(err) = val["error"]["message"].as_str() {
            self.error = Some(err.to_string());
        }
    }

    fn append_text(&mut self, text: &str, current_time_ms: u64) {
        if self.first_token_time_ms.is_none() {
            self.first_token_time_ms = Some(current_time_ms);
        }
        let chunk_tokens = LlmStudio::count_tokens(text);
        self.total_tokens += chunk_tokens;
        if self.accumulated_text.len() < 10_000_000 {
            self.accumulated_text.push_str(text);
        }
    }

    fn metrics(&self, current_time_ms: u64) -> SseStreamMetrics {
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
            tool_calls: self.tool_calls.clone(),
            finish_reason: self.finish_reason.clone(),
            error: self.error.clone(),
        }
    }
}

fn text_delta(val: &serde_json::Value) -> Option<&str> {
    // OpenAI / generic delta content
    let mut text_delta = val["choices"][0]["delta"]["content"]
        .as_str()
        .or_else(|| val["choices"][0]["text"].as_str())
        .or_else(|| val["delta"]["text"].as_str())
        .or_else(|| val["content"].as_str());

    // Anthropic Messages API: {"type":"content_block_delta", "delta":{"type":"text_delta","text":"..."}}
    if text_delta.is_none() && val["type"] == "content_block_delta" {
        text_delta = val["delta"]["text"].as_str();
    }

    // Ollama Native Stream: {"response":"..."}
    if text_delta.is_none() {
        text_delta = val["response"].as_str();
    }

    // Gemini REST Stream: candidates[0].content.parts[0].text
    if text_delta.is_none() {
        text_delta = val["candidates"][0]["content"]["parts"][0]["text"].as_str();
    }
    text_delta
}
