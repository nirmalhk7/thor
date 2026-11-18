use super::{AiEngine, LlmRequestSummary, PromptDiff};
use crate::dx::llm_studio::LlmStudio;

impl AiEngine {
    // 1. LLM Inspector ------------------------------------------------------
    pub fn inspect_request(&self, body: &serde_json::Value) -> LlmRequestSummary {
        let model = body
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown")
            .to_string();
        let temperature = body.get("temperature").and_then(|t| t.as_f64());
        let streaming = body
            .get("stream")
            .and_then(|s| s.as_bool())
            .unwrap_or(false);
        let mut roles = Vec::new();
        let mut prompt_tokens = 0;
        let mut message_count = 0;
        if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
            message_count = messages.len();
            for m in messages {
                if let Some(role) = m.get("role").and_then(|r| r.as_str()) {
                    if !roles.contains(&role.to_string()) {
                        roles.push(role.to_string());
                    }
                }
                if let Some(content) = m.get("content").and_then(|c| c.as_str()) {
                    prompt_tokens += LlmStudio::count_tokens(content);
                }
            }
        } else if let Some(prompt) = body.get("prompt").and_then(|p| p.as_str()) {
            message_count = 1;
            prompt_tokens = LlmStudio::count_tokens(prompt);
        }
        LlmRequestSummary {
            model,
            message_count,
            roles,
            prompt_tokens,
            temperature,
            streaming,
        }
    }

    // 3. Prompt Diff --------------------------------------------------------
    pub fn prompt_diff(&self, before: &str, after: &str) -> PromptDiff {
        let a_lines: Vec<&str> = before.lines().collect();
        let b_lines: Vec<&str> = after.lines().collect();
        let a_set: std::collections::HashSet<&str> = a_lines.iter().copied().collect();
        let b_set: std::collections::HashSet<&str> = b_lines.iter().copied().collect();
        let added = b_lines
            .iter()
            .filter(|l| !a_set.contains(*l))
            .map(|s| s.to_string())
            .collect();
        let removed = a_lines
            .iter()
            .filter(|l| !b_set.contains(*l))
            .map(|s| s.to_string())
            .collect();
        let token_delta =
            LlmStudio::count_tokens(after) as i64 - LlmStudio::count_tokens(before) as i64;
        let common = a_set.intersection(&b_set).count();
        let union = a_set.union(&b_set).count().max(1);
        PromptDiff {
            added,
            removed,
            token_delta,
            similarity: common as f64 / union as f64,
        }
    }
}
