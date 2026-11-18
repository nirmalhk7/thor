use super::{ok, opt_str, opt_vec, req, reqv, FeatureEngine, Value};

pub(in crate::features) fn promote_finding(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().promote_to_finding_bundle(
        opt_str(p, "title").unwrap_or_default().as_str(),
        opt_str(p, "target").unwrap_or_default().as_str(),
        opt_vec(p, "traces").unwrap_or_default(),
    ))
}

pub(in crate::features) fn mass_assignment_diff(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.security().mass_assignment_diff(&req(p, "diff")?))
}

pub(in crate::features) fn sri_supply_chain_auditor(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.security().sri_supply_chain_auditor(&reqv(p, "scripts")?))
}
