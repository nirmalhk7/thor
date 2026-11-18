use super::{AiEngine, McpMessage, McpTrafficReport};
use std::collections::BTreeMap;

impl AiEngine {
    pub fn mcp_traffic_lens(&self, messages: &[McpMessage]) -> McpTrafficReport {
        let mut methods: BTreeMap<String, usize> = BTreeMap::new();
        let mut errors = 0usize;
        let mut lat = 0u64;
        let mut issues = Vec::new();
        for m in messages {
            *methods.entry(m.method.clone()).or_default() += 1;
            lat += m.latency_ms;
            if m.is_error {
                errors += 1;
            }
            if m.bytes > 1_000_000 {
                issues.push(format!("{} message {} bytes", m.method, m.bytes));
            }
        }
        let avg = if messages.is_empty() {
            0.0
        } else {
            lat as f64 / messages.len() as f64
        };
        if errors > 0 {
            issues.push(format!("{errors} MCP error messages"));
        }
        McpTrafficReport {
            messages: messages.len(),
            errors,
            methods,
            avg_latency_ms: avg,
            issues,
        }
    }

    /// Accept Thor `src/mcp` JSON-RPC request/response shapes (`method`, `id`,
    /// optional `error` / `result`) and fold into [`McpMessage`]s.
    ///
    /// Pairs request/response by `id`, computes latency when both timestamps are
    /// present, and labels unmatched requests/responses.
    pub fn mcp_messages_from_jsonrpc(frames: &[serde_json::Value]) -> Vec<McpMessage> {
        let mut out = Vec::new();
        let mut pending_req: BTreeMap<String, (String, Option<u64>, usize)> = BTreeMap::new();
        for v in frames {
            let id = v.get("id").and_then(|i| {
                i.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| i.as_u64().map(|n| n.to_string()))
            });
            let ts = v
                .get("timestamp_ms")
                .or_else(|| v.get("ts"))
                .and_then(|t| t.as_u64());
            let bytes = v.to_string().len();
            let is_error = v.get("error").is_some()
                || v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
            let has_method = v.get("method").and_then(|m| m.as_str()).is_some();
            let has_result = v.get("result").is_some() || v.get("error").is_some();

            match id {
                Some(id) if has_method && !has_result => {
                    let method = v
                        .get("method")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    pending_req.insert(id, (method, ts, bytes));
                }
                Some(id) if has_result => {
                    if let Some((method, start_ts, req_bytes)) = pending_req.remove(&id) {
                        let latency = match (start_ts, ts) {
                            (Some(a), Some(b)) => b.saturating_sub(a),
                            _ => v.get("latency_ms").and_then(|l| l.as_u64()).unwrap_or(0),
                        };
                        out.push(McpMessage {
                            method,
                            id: Some(id),
                            is_error,
                            bytes: req_bytes + bytes,
                            latency_ms: latency,
                        });
                    } else {
                        out.push(McpMessage {
                            method: "unmatched-response".into(),
                            id: Some(id),
                            is_error,
                            bytes,
                            latency_ms: 0,
                        });
                    }
                }
                None if has_method => {
                    out.push(McpMessage {
                        method: v
                            .get("method")
                            .and_then(|m| m.as_str())
                            .unwrap_or("notification")
                            .to_string(),
                        id: None,
                        is_error,
                        bytes,
                        latency_ms: v.get("latency_ms").and_then(|l| l.as_u64()).unwrap_or(0),
                    });
                }
                _ => {}
            }
        }
        for (id, (method, _, bytes)) in pending_req {
            out.push(McpMessage {
                method: format!("{method} (unmatched-request)"),
                id: Some(id),
                is_error: false,
                bytes,
                latency_ms: 0,
            });
        }
        out
    }
}
