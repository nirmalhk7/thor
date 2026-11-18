use super::{json, ok, op, opt_str, req, reqv, FeatureEngine, Value};

pub(in crate::features) fn hdr_histogram_helper(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    let hist = f
        .perf()
        .histogram_from_samples(&req::<Vec<f64>>(p, "samples")?);
    ok(json!({
        "histogram": hist,
        "percentiles": f.perf().percentile_table(&hist),
    }))
}

pub(in crate::features) fn flame_graph(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let folded: Vec<String> = req(p, "folded")?;
    if op(p) == "profile" {
        ok(f.perf()
            .promote_to_profile(&folded, opt_str(p, "name").unwrap_or_default().as_str()))
    } else {
        ok(f.perf().build_flame_graph(&folded))
    }
}

pub(in crate::features) fn long_task_radar(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.perf().long_task_radar(&reqv(p, "tasks")?))
}

pub(in crate::features) fn main_thread_block_map(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf().main_thread_block_map(&reqv(p, "slices")?))
}

pub(in crate::features) fn continuous_profiler(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf().continuous_profile(&reqv(p, "samples")?))
}

pub(in crate::features) fn performance_sla(f: &FeatureEngine, _p: &Value) -> Result<Value, String> {
    let results: Vec<Value> = f
        .sla()
        .measure_feature_paths()
        .into_iter()
        .map(|r| {
            json!({
                "id": r.sla.id,
                "feature": format!("{:?}", r.sla.feature),
                "description": r.sla.description,
                "threshold_us": r.effective_threshold().as_micros() as u64,
                "per_op_us": r.per_op.as_micros() as u64,
                "passed": r.passed,
            })
        })
        .collect();
    ok(results)
}
