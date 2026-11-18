use super::ai;
use super::{
    ok, opt, opt_str, opt_val, opt_vec, reqv, tool_events_from_tamper, AppState, FeatureEngine,
    Value,
};

pub(in crate::features) fn llm_inspector(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let body = opt_val(p, "body");
    let summary = f.ai().inspect_request(&body);
    let mut out = serde_json::to_value(&summary).map_err(|e| format!("serialize failed: {e}"))?;
    if let Some(usage) = body.get("usage").or_else(|| p.get("usage")).cloned() {
        let model = summary.model.clone();
        let ev = ai::AiEngine::cache_hit_event_from_usage(&usage, &model, "inspect");
        let ledger = f.ai().cache_hit_ledger(&[ev]);
        if let Some(obj) = out.as_object_mut() {
            obj.insert(
                "cache_hit_ledger".into(),
                serde_json::to_value(ledger).unwrap_or_default(),
            );
        }
    }
    Ok(out)
}

pub(in crate::features) fn streaming_waterfall(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().streaming_waterfall(&reqv(p, "chunks")?))
}

pub(in crate::features) fn tool_call_forensics(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if let Some(sse) = opt_str(p, "sse") {
        let events = ai::AiEngine::tool_call_events_from_sse(&sse);
        ok(f.ai().tool_call_forensics(&events))
    } else if opt::<bool>(p, "from_tamper").unwrap_or(false)
        || opt::<bool>(p, "from_active_tab").unwrap_or(false)
    {
        let events = tool_events_from_tamper(AppState::global().tamper().get_history());
        ok(f.ai().tool_call_forensics(&events))
    } else {
        ok(f.ai().tool_call_forensics(&reqv(p, "events")?))
    }
}

pub(in crate::features) fn mcp_traffic_lens(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if let Some(frames) = opt_vec::<serde_json::Value>(p, "jsonrpc") {
        let msgs = ai::AiEngine::mcp_messages_from_jsonrpc(&frames);
        ok(f.ai().mcp_traffic_lens(&msgs))
    } else {
        ok(f.ai().mcp_traffic_lens(&reqv(p, "messages")?))
    }
}
