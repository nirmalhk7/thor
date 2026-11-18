use super::ai;
use super::{ok, opt, opt_str, reqv, stream_chunks_from_tamper, AppState, FeatureEngine, Value};

pub(in crate::features) fn cache_hit_ledger(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if let Some(usage) = p.get("usage").cloned() {
        let model = opt_str(p, "model").unwrap_or_else(|| "gpt-4o".into());
        let key = opt_str(p, "key").unwrap_or_else(|| "usage".into());
        let ev = ai::AiEngine::cache_hit_event_from_usage(&usage, &model, &key);
        ok(f.ai().cache_hit_ledger(&[ev]))
    } else {
        ok(f.ai().cache_hit_ledger(&reqv(p, "events")?))
    }
}

pub(in crate::features) fn token_waste_auditor(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().token_waste_auditor(&reqv(p, "segments")?))
}

pub(in crate::features) fn streaming_slo_gates(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if let Some(wf) = opt::<ai::StreamWaterfall>(p, "waterfall") {
        ok(f.ai().streaming_slo_gates_from_waterfall(
            &wf,
            opt(p, "ttft_budget_ms").unwrap_or(500),
            opt(p, "min_tokens_per_sec").unwrap_or(20.0),
            opt(p, "max_gap_budget_ms").unwrap_or(200),
        ))
    } else if opt::<bool>(p, "from_tamper").unwrap_or(false)
        || opt::<bool>(p, "from_active_tab").unwrap_or(false)
    {
        let chunks = stream_chunks_from_tamper(AppState::global().tamper().get_history());
        let wf = f.ai().streaming_waterfall(&chunks);
        ok(f.ai().streaming_slo_gates_from_waterfall(
            &wf,
            opt(p, "ttft_budget_ms").unwrap_or(500),
            opt(p, "min_tokens_per_sec").unwrap_or(20.0),
            opt(p, "max_gap_budget_ms").unwrap_or(200),
        ))
    } else if let Some(input) = opt::<ai::StreamingSloInput>(p, "input") {
        ok(f.ai().streaming_slo_gates(&input))
    } else {
        // Flat demo payload: ttft_ms / tokens_per_sec
        let input = ai::StreamingSloInput {
            ttft_ms: opt(p, "ttft_ms").unwrap_or(120),
            tokens_per_sec: opt(p, "tokens_per_sec").unwrap_or(40.0),
            max_gap_ms: opt(p, "max_gap_ms").unwrap_or(50),
            ttft_budget_ms: opt(p, "ttft_budget_ms").unwrap_or(500),
            min_tokens_per_sec: opt(p, "min_tokens_per_sec").unwrap_or(20.0),
            max_gap_budget_ms: opt(p, "max_gap_budget_ms").unwrap_or(200),
        };
        ok(f.ai().streaming_slo_gates(&input))
    }
}

pub(in crate::features) fn router_decision_trace(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().router_decision_trace(&reqv(p, "decisions")?))
}

pub(in crate::features) fn protocol_quirk_diff(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().protocol_quirk_diff(&reqv(p, "samples")?))
}

pub(in crate::features) fn local_cloud_parity(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().local_cloud_parity(&reqv(p, "runs")?))
}
