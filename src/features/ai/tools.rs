use super::{AgentStep, AiEngine, ToolCallEvent, ToolCallForensicsReport};
use std::collections::BTreeMap;

impl AiEngine {
    // ---- Wave-2 AI --------------------------------------------------------

    pub fn tool_call_forensics(&self, events: &[ToolCallEvent]) -> ToolCallForensicsReport {
        let mut by_tool: BTreeMap<String, usize> = BTreeMap::new();
        let mut failures = 0usize;
        let mut lat_sum = 0u64;
        let mut issues = Vec::new();
        for e in events {
            *by_tool.entry(e.name.clone()).or_default() += 1;
            lat_sum += e.latency_ms;
            if !e.success {
                failures += 1;
                issues.push(format!(
                    "{} failed: {}",
                    e.name,
                    e.error.clone().unwrap_or_else(|| "unknown".into())
                ));
            }
            if e.arguments_json.len() > 8_000 {
                issues.push(format!(
                    "{} huge arguments ({} bytes)",
                    e.name,
                    e.arguments_json.len()
                ));
            }
        }
        let avg = if events.is_empty() {
            0.0
        } else {
            lat_sum as f64 / events.len() as f64
        };
        ToolCallForensicsReport {
            calls: events.len(),
            failures,
            by_tool,
            avg_latency_ms: avg,
            issues,
        }
    }

    /// Parse OpenAI-style SSE (`data: {choices[0].delta.tool_calls}`) into tool events.
    ///
    /// Call-only streams never fabricate success: `success` stays `false` until a
    /// matching `tool_result` / tool-role message merges. Latency uses timestamps
    /// when present on call + result; otherwise `0`.
    pub fn tool_call_events_from_sse(sse: &str) -> Vec<ToolCallEvent> {
        super::tool_calls::parse(sse)
    }

    /// Build an agent-trace graph from SSE tool_calls / tool_use payloads.
    pub fn agent_steps_from_sse(sse: &str) -> Vec<AgentStep> {
        let events = Self::tool_call_events_from_sse(sse);
        let mut steps = Vec::new();
        for e in events {
            steps.push(AgentStep {
                kind: "action".into(),
                tool: Some(e.name.clone()),
                content: e.arguments_json.clone(),
            });
            steps.push(AgentStep {
                kind: "observation".into(),
                tool: Some(e.name),
                content: if e.success {
                    format!("ok latency_ms={}", e.latency_ms)
                } else {
                    e.error.unwrap_or_else(|| "pending: no tool_result".into())
                },
            });
        }
        steps
    }
}
