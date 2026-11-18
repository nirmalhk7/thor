use super::{ok, opt, reqv, FeatureEngine, Value};

pub(in crate::features) fn conn_pool_inspector(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf().conn_pool_inspect(&reqv(p, "entries")?))
}

pub(in crate::features) fn http3_path_audit(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.perf().http3_path_audit(&reqv(p, "samples")?))
}

pub(in crate::features) fn compression_budget(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf().compression_budget(
        &reqv(p, "entries")?,
        opt(p, "max_inflation_ratio").unwrap_or(4.0),
    ))
}

pub(in crate::features) fn media_weight_gate(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf().media_weight_gate(
        &reqv(p, "resources")?,
        opt(p, "max_image_bytes").unwrap_or(1_500_000),
        opt(p, "max_video_bytes").unwrap_or(5_000_000),
        opt(p, "max_lcp_media_bytes").unwrap_or(500_000),
    ))
}
