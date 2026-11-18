use super::qa;
use super::{json, ok, op, opt, opt_str, opt_vec, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn resilient_locators(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa().rank_locators(&req(p, "signals")?))
}

pub(in crate::features) fn flake_radar(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if let Some(scenario) = opt::<crate::load::ThorScenario>(p, "scenario") {
        ok(f.qa()
            .flake_radar_replay(&scenario, opt(p, "n").unwrap_or(3)))
    } else {
        ok(f.qa().flake_radar(
            opt_str(p, "test").unwrap_or_default().as_str(),
            &reqv(p, "results")?,
        ))
    }
}

pub(in crate::features) fn visual_regression(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    let baseline: Vec<u8> = req(p, "baseline")?;
    let candidate: Vec<u8> = req(p, "candidate")?;
    let threshold = opt(p, "threshold").unwrap_or(0.01);
    match opt_vec::<qa::MaskRegion>(p, "mask_regions") {
        Some(masks) => ok(f.qa().visual_regression_masked(
            &baseline,
            &candidate,
            opt(p, "width").unwrap_or(0),
            threshold,
            &masks,
        )),
        None => ok(f.qa().visual_regression(&baseline, &candidate, threshold)),
    }
}

pub(in crate::features) fn identity_fixtures(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    match op(p) {
        "add" => {
            let fixture: qa::IdentityFixture = req(p, "fixture")?;
            let name = fixture.name.clone();
            f.qa().add_fixture(fixture);
            ok(json!({ "setup": f.qa().fixture_setup_script(&name) }))
        }
        _ => ok(json!({
            "setup": f.qa().fixture_setup_script(str_req(p, "name")?)
        })),
    }
}

pub(in crate::features) fn data_driven_scenarios(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa()
        .expand_data_driven(str_req(p, "template")?, &reqv(p, "dataset")?))
}

pub(in crate::features) fn real_vitals_gates(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa().vitals_gate(&req(p, "vitals")?, &req(p, "budgets")?))
}
