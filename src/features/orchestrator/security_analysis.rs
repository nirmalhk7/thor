use super::{ok, opt_vec, req, reqv, FeatureEngine, Value};

pub(in crate::features) fn response_diff(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().diff_responses(&req(p, "a")?, &req(p, "b")?))
}

pub(in crate::features) fn header_analyzer(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().analyze_headers(&req(p, "headers")?))
}

pub(in crate::features) fn surface_crawler(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().crawl_surface_scoped(
        &req::<Vec<(String, String)>>(p, "discovered")?,
        &opt_vec(p, "host_allowlist").unwrap_or_default(),
    ))
}

pub(in crate::features) fn storage_auditor(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().audit_storage(&reqv(p, "items")?))
}
