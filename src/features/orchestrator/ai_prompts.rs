use super::{json, ok, op, opt, opt_str, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn prompt_diff(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match op(p) {
        "record" => ok(f
            .ai()
            .record_prompt_version(str_req(p, "name")?, str_req(p, "content")?)),
        "timeline" => ok(f.ai().prompt_timeline(str_req(p, "name")?)),
        _ => ok(f
            .ai()
            .prompt_diff(str_req(p, "before")?, str_req(p, "after")?)),
    }
}

pub(in crate::features) fn cost_dashboard(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match op(p) {
        "log" => {
            let model = str_req(p, "model")?;
            let pt = opt(p, "prompt_tokens").unwrap_or(0);
            let ct = opt(p, "completion_tokens").unwrap_or(0);
            match opt_str(p, "experiment_id") {
                Some(exp) => ok(f.ai().log_cost_for_experiment(&exp, model, pt, ct)),
                None => ok(f.ai().log_cost(model, pt, ct)),
            }
        }
        "by_experiment" => ok(f.ai().cost_by_experiment()),
        "reset" => {
            f.ai().reset_costs();
            ok(json!({ "reset": true }))
        }
        _ => ok(f.ai().cost_summary()),
    }
}

pub(in crate::features) fn context_heatmap(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.ai()
        .context_heatmap(req(p, "window_tokens")?, &reqv(p, "segments")?))
}

pub(in crate::features) fn rag_inspector(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.ai()
        .inspect_rag(&reqv(p, "chunks")?, str_req(p, "answer")?))
}

pub(in crate::features) fn injection_scanner(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().scan_injection(str_req(p, "input")?))
}
