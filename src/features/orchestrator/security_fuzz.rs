use super::security;
use super::{ok, opt_str, opt_val, req, str_req, FeatureEngine, Value};

pub(in crate::features) fn fuzz_repeater(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let category: security::FuzzCategory = req(p, "category")?;
    match f.security().build_fuzz_cases_gated(
        str_req(p, "template")?,
        category,
        str_req(p, "target_url")?,
    ) {
        Ok(cases) => ok(cases),
        Err(e) => Err(e),
    }
}

pub(in crate::features) fn csrf_analyzer(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().analyze_csrf(&req(p, "context")?))
}

pub(in crate::features) fn sink_pollution(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security()
        .detect_sinks(&opt_val(p, "json"), opt_str(p, "script").as_deref()))
}

pub(in crate::features) fn tls_inspector(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().inspect_tls(&req(p, "info")?))
}
