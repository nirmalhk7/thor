use super::frontend;
use super::{ok, opt, opt_str, opt_vec, req, reqv, FeatureEngine, Value};

pub(in crate::features) fn css_cascade(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.frontend().resolve_cascade(&reqv(p, "declarations")?))
}

pub(in crate::features) fn design_tokens(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let declarations: Vec<frontend::CssDeclaration> = reqv(p, "declarations")?;
    match opt_vec::<String>(p, "canonical") {
        Some(canon) => ok(f.frontend().token_drift(&declarations, &canon)),
        None => ok(f.frontend().extract_design_tokens(&declarations)),
    }
}

pub(in crate::features) fn css_coverage_map(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let raw = f.evaluate_on_active(frontend::FrontendEngine::css_coverage_probe_script())?;
        let rules: Vec<frontend::CssRuleCoverage> = serde_json::from_str(&raw)
            .map_err(|e| format!("invalid CSS coverage probe JSON: {e}"))?;
        ok(f.frontend().css_coverage_map(&rules))
    } else if let Some(json) = opt_str(p, "coverage_json") {
        ok(frontend::FrontendEngine::css_coverage_from_take_coverage_delta(&json))
    } else {
        ok(f.frontend().css_coverage_map(&reqv(p, "rules")?))
    }
}

pub(in crate::features) fn css_variable_inspector(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend()
        .css_variable_inspector(&req(p, "definitions")?, &reqv(p, "references")?))
}

pub(in crate::features) fn css_containment_advisor(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend().css_containment_advisor(&reqv(p, "nodes")?))
}

pub(in crate::features) fn selector_specificity_budget(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend().selector_specificity_budget(
        &reqv(p, "selectors")?,
        opt(p, "budget").unwrap_or([0, 3, 2]),
    ))
}
