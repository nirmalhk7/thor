use super::perf_features;
use super::{ok, opt, opt_vec, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn trace_diff(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.perf().diff_traces(
        &req(p, "before")?,
        &req(p, "after")?,
        opt(p, "regression_threshold_pct").unwrap_or(10.0),
    ))
}

pub(in crate::features) fn waterfall_forensics(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if opt::<bool>(p, "from_network").unwrap_or(false)
        || opt::<bool>(p, "from_active_tab").unwrap_or(false)
    {
        let initiated = f.cdp().initiated_resources();
        if !initiated.is_empty() {
            ok(f.perf().blocking_chain(&initiated))
        } else {
            ok(f.perf().analyze_waterfall(&f.cdp().resource_timings()))
        }
    } else {
        match opt_vec::<perf_features::InitiatedResource>(p, "initiated") {
            Some(res) => ok(f.perf().blocking_chain(&res)),
            None => ok(f.perf().analyze_waterfall(&reqv(p, "resources")?)),
        }
    }
}

pub(in crate::features) fn memory_leak_hunter(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf().detect_leak(&reqv(p, "samples")?))
}

pub(in crate::features) fn cdn_cache(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.perf().analyze_cache(&reqv(p, "entries")?))
}

pub(in crate::features) fn third_party_attribution(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf()
        .attribute_third_party(str_req(p, "first_party")?, &reqv(p, "resources")?))
}
