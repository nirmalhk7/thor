use super::perf_features;
use super::{json, ok, opt, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn synthetic_monitor(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if let Some(session) = opt::<crate::dx::test_generator::RecordedSession>(p, "session") {
        let n = f.perf().synthetic_from_session(&session);
        ok(json!({
            "checks_recorded": n,
            "summary": f.perf().synthetic_summary(&session.target_url)
        }))
    } else if let Some(check) = opt::<perf_features::SyntheticCheck>(p, "check") {
        let target = check.target.clone();
        f.perf().record_synthetic(check);
        ok(f.perf().synthetic_summary(&target))
    } else {
        ok(f.perf().synthetic_summary(str_req(p, "target")?))
    }
}

pub(in crate::features) fn bf_cache_probe(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.perf().bfcache_probe(&reqv(p, "samples")?))
}

pub(in crate::features) fn freeze_resume_bench(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf().freeze_resume_bench(&reqv(p, "samples")?))
}

pub(in crate::features) fn speculation_lab(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.perf().speculation_lab(&reqv(p, "trials")?))
}
