use super::ai;
use crate::net::tamper::{InterceptedRequest, InterceptedResponse};

type History = Vec<(InterceptedRequest, Option<InterceptedResponse>)>;

pub(super) fn tool_events_from_tamper(history: History) -> Vec<ai::ToolCallEvent> {
    let mut events = Vec::new();
    for (request, response) in history {
        let Some(body) = request
            .body
            .as_deref()
            .or(response.as_ref().and_then(|r| r.body.as_deref()))
        else {
            continue;
        };
        if body.contains("tool_calls") || body.contains("function_call") {
            events.extend(ai::AiEngine::tool_call_events_from_sse(body));
        }
    }
    events
}

pub(super) fn stream_chunks_from_tamper(history: History) -> Vec<ai::StreamChunk> {
    let mut chunks = Vec::new();
    let mut time = 0u64;
    for (_request, response) in history {
        let Some(body) = response.and_then(|r| r.body) else {
            continue;
        };
        if !body.contains("data:") {
            continue;
        }
        for line in body.lines().filter(|line| line.starts_with("data:")) {
            chunks.push(ai::StreamChunk {
                raw: line.to_string(),
                arrival_ms: time,
            });
            time += 25;
        }
    }
    chunks
}
