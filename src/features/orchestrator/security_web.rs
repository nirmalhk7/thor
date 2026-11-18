use super::security;
use super::{ok, opt_str, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn dom_xss_taint_map(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if let Some(json) = opt_str(p, "flows_json") {
        let flows = security::SecurityEngine::parse_dom_xss_taint_flows(&json);
        ok(f.security().dom_xss_taint_map(&flows))
    } else {
        ok(f.security().dom_xss_taint_map(&reqv(p, "flows")?))
    }
}

pub(in crate::features) fn cors_deep_probe(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().cors_deep_probe(&reqv(p, "probes")?))
}

pub(in crate::features) fn csp_policy_forensics(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.security().csp_policy_forensics(str_req(p, "policy")?))
}

pub(in crate::features) fn oauth_oidc_misconfig(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.security().oauth_oidc_misconfig(&req(p, "config")?))
}

pub(in crate::features) fn cookie_session_forensics(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.security().cookie_session_forensics(&reqv(p, "cookies")?))
}
