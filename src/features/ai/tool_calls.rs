use super::ToolCallEvent;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Default)]
struct Pending {
    name: String,
    args: String,
    call_id: Option<String>,
    success: bool,
    latency_ms: u64,
    error: Option<String>,
    start_ms: Option<u64>,
}

#[derive(Default)]
struct ToolEvents {
    by_index: BTreeMap<usize, Pending>,
    id_to_index: BTreeMap<String, usize>,
}

pub(super) fn parse(sse: &str) -> Vec<ToolCallEvent> {
    let mut events = ToolEvents::default();
    for line in sse.lines() {
        let data = line
            .strip_prefix("data:")
            .map(str::trim)
            .unwrap_or(line.trim());
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        events.frame(&value);
    }
    events
        .by_index
        .into_values()
        .map(Pending::into_event)
        .collect()
}

impl ToolEvents {
    fn frame(&mut self, value: &Value) {
        let timestamp = value
            .get("created_ms")
            .or_else(|| value.get("timestamp_ms"))
            .or_else(|| value.get("created"))
            .and_then(|t| t.as_u64());
        self.openai(value, timestamp);
        self.top_level_result(value, timestamp);
        self.tool_role(value, timestamp);
        self.anthropic(value, timestamp);
    }

    fn mark_result(&mut self, call_id: Option<&str>, is_error: bool, end_ms: Option<u64>) {
        let index = call_id
            .and_then(|id| self.id_to_index.get(id).copied())
            .or_else(|| self.by_index.keys().next_back().copied());
        let Some(index) = index else {
            return;
        };
        let Some(pending) = self.by_index.get_mut(&index) else {
            return;
        };
        pending.success = !is_error;
        if is_error {
            pending.error = Some(
                pending
                    .error
                    .clone()
                    .unwrap_or_else(|| "tool_result error".into()),
            );
        }
        if let (Some(start), Some(end)) = (pending.start_ms, end_ms) {
            if end >= start {
                pending.latency_ms = end - start;
            }
        }
    }

    fn openai(&mut self, value: &Value, timestamp: Option<u64>) {
        if let Some(calls) = value
            .pointer("/choices/0/delta/tool_calls")
            .or_else(|| value.pointer("/choices/0/message/tool_calls"))
            .and_then(|x| x.as_array())
        {
            for call in calls {
                self.openai_call(call, timestamp);
            }
        }
    }

    fn openai_call(&mut self, call: &Value, timestamp: Option<u64>) {
        let index = call.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
        let name = call
            .pointer("/function/name")
            .or_else(|| call.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let args = call
            .pointer("/function/arguments")
            .or_else(|| call.get("input").or_else(|| call.get("arguments")))
            .map(|argument| {
                if argument.is_string() {
                    argument.as_str().unwrap_or("").to_string()
                } else {
                    argument.to_string()
                }
            })
            .unwrap_or_default();
        let call_id = call.get("id").and_then(|i| i.as_str()).map(str::to_string);
        let entry = self.by_index.entry(index).or_default();
        if !name.is_empty() {
            entry.name = name;
        }
        entry.args.push_str(&args);
        if let Some(id) = call_id {
            self.id_to_index.insert(id.clone(), index);
            entry.call_id = Some(id);
        }
        if entry.start_ms.is_none() {
            entry.start_ms = timestamp;
        }
        entry.success = false;
    }

    fn top_level_result(&mut self, value: &Value, timestamp: Option<u64>) {
        if value.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
            let id = value
                .get("tool_call_id")
                .or_else(|| value.get("tool_use_id"))
                .and_then(|i| i.as_str());
            let is_error = value
                .get("is_error")
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            self.mark_result(id, is_error, timestamp);
        }
    }

    fn tool_role(&mut self, value: &Value, timestamp: Option<u64>) {
        let role = value
            .pointer("/choices/0/delta/role")
            .or_else(|| value.pointer("/choices/0/message/role"))
            .or_else(|| value.get("role"))
            .and_then(|r| r.as_str());
        if role == Some("tool") {
            let id = value
                .get("tool_call_id")
                .or_else(|| value.pointer("/choices/0/message/tool_call_id"))
                .and_then(|i| i.as_str());
            self.mark_result(id, false, timestamp);
        }
    }

    fn anthropic(&mut self, value: &Value, timestamp: Option<u64>) {
        if let Some(blocks) = value.get("content").and_then(|c| c.as_array()) {
            for block in blocks {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("tool_use") => self.anthropic_call(block, timestamp),
                    Some("tool_result") => self.anthropic_result(block, timestamp),
                    _ => {}
                }
            }
        }
    }

    fn anthropic_call(&mut self, block: &Value, timestamp: Option<u64>) {
        let name = block
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("tool")
            .to_string();
        let args = block
            .get("input")
            .map(|i| i.to_string())
            .unwrap_or_else(|| "{}".into());
        let call_id = block.get("id").and_then(|i| i.as_str()).map(str::to_string);
        let index = self.by_index.len();
        if let Some(ref id) = call_id {
            self.id_to_index.insert(id.clone(), index);
        }
        self.by_index.insert(
            index,
            Pending {
                name,
                args,
                call_id,
                success: false,
                latency_ms: 0,
                error: None,
                start_ms: timestamp,
            },
        );
    }

    fn anthropic_result(&mut self, block: &Value, timestamp: Option<u64>) {
        let id = block
            .get("tool_use_id")
            .or_else(|| block.get("tool_call_id"))
            .and_then(|i| i.as_str());
        let is_error = block
            .get("is_error")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);
        self.mark_result(id, is_error, timestamp);
    }
}

impl Pending {
    fn into_event(self) -> ToolCallEvent {
        ToolCallEvent {
            name: if self.name.is_empty() {
                "unknown".into()
            } else {
                self.name
            },
            arguments_json: self.args,
            success: self.success,
            latency_ms: self.latency_ms,
            error: if self.success {
                None
            } else {
                self.error.or_else(|| Some("awaiting tool_result".into()))
            },
        }
    }
}
