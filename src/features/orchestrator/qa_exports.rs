use super::qa;
use super::{json, ok, op, opt, opt_str, req, reqv, str_req, AppState, FeatureEngine, Value};

pub(in crate::features) fn contract_testing(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let interactions = reqv(p, "interactions")?;
    if interactions.is_empty() {
        return Err("Add at least one expected interaction in interactions.".into());
    }
    ok(f.qa().verify_contract(&interactions, &reqv(p, "actuals")?))
}

pub(in crate::features) fn deflaked_export(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let session: crate::dx::test_generator::RecordedSession = if let Some(s) = opt(p, "session") {
        s
    } else if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let url = f
            .evaluate_on_active("location.href")
            .unwrap_or_else(|_| "about:blank".into());
        let mut session = crate::dx::test_generator::RecordedSession::new("live-export", url);
        for (req, _) in AppState::global().tamper().get_history() {
            session
                .traces
                .push(crate::dx::test_generator::NetworkTrace {
                    method: req.method,
                    url: req.url,
                    status: 0,
                    headers: req.headers,
                    body: req.body,
                });
        }
        session
    } else {
        req(p, "session")?
    };
    let playwright = if let Some(obs) = opt::<qa::OracleObservation>(p, "observation") {
        f.qa().export_with_oracles(&session, &obs)
    } else {
        f.qa().export_with_oracles(
            &session,
            &qa::OracleObservation {
                dom_digest: None,
                status: Some(200),
                headers: Default::default(),
                vitals: None,
                storage: Default::default(),
            },
        )
    };
    ok(json!({ "playwright": playwright }))
}

pub(in crate::features) fn ci_bundle(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let session: crate::dx::test_generator::RecordedSession = if let Some(s) = opt(p, "session") {
        s
    } else if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let url = f
            .evaluate_on_active("location.href")
            .unwrap_or_else(|_| "about:blank".into());
        let mut session = crate::dx::test_generator::RecordedSession::new("live-ci", url);
        for (req, resp) in AppState::global().tamper().get_history() {
            session
                .traces
                .push(crate::dx::test_generator::NetworkTrace {
                    method: req.method,
                    url: req.url,
                    status: resp.map(|r| r.status_code).unwrap_or(0),
                    headers: req.headers,
                    body: req.body,
                });
        }
        session
    } else {
        req(p, "session")?
    };
    let obs = opt::<qa::OracleObservation>(p, "observation");
    let mut bundle = f.qa().export_ci_bundle_composed(&session, obs.as_ref());
    // P0-6: orch path must never serialize empty promote artifacts.
    if bundle.playwright_ts.trim().is_empty() {
        bundle.playwright_ts = String::from(
                        "// Thor CI bundle stub\nimport { test } from '@playwright/test';\ntest('stub', async () => {});\n",
                    );
    }
    if bundle.hermetic_ts.trim().is_empty() {
        bundle.hermetic_ts = String::from(
                        "// Thor hermetic fixture stub\nimport { test } from '@playwright/test';\ntest.describe('hermetic stub', () => {});\n",
                    );
    }
    if bundle.quarantine_filter.trim().is_empty() {
        bundle.quarantine_filter =
            crate::dx::test_generator::TestGenerator::ci_quarantine_filter(&[]);
    }
    ok(bundle)
}

pub(in crate::features) fn time_travel_trace(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    match op(p) {
        "record" => {
            let test = str_req(p, "test")?;
            let step = opt_str(p, "step").unwrap_or_default();
            let action = opt_str(p, "action").unwrap_or_default();
            let dom = if opt::<bool>(p, "from_active_tab").unwrap_or(false)
                || opt_str(p, "dom").unwrap_or_default().is_empty()
            {
                f.evaluate_on_active(
                            "(function(){ return document.documentElement ? document.documentElement.outerHTML : ''; })()",
                        )
                        .unwrap_or_default()
            } else {
                opt_str(p, "dom").unwrap_or_default()
            };
            ok(json!({
                "index": f.qa().record_trace_frame(
                    test,
                    step.as_str(),
                    action.as_str(),
                    &dom,
                )
            }))
        }
        "export" => ok(f.qa().export_playwright_trace(str_req(p, "test")?)?),
        "at" => ok(json!({
            "frame": f.qa().trace_frame_at(str_req(p, "test")?, req(p, "index")?)
        })),
        _ => ok(f.qa().trace_frames(str_req(p, "test")?)),
    }
}

pub(in crate::features) fn determinism_harness(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if op(p) == "controls" {
        ok(f.qa().determinism_controls(
            opt(p, "seed").unwrap_or(42),
            opt(p, "epoch_ms").unwrap_or(0),
        ))
    } else {
        ok(f.qa().determinism(&reqv(p, "outputs")?))
    }
}

pub(in crate::features) fn authz_matrix_tests(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    let matrix: Vec<(String, String, bool)> = req(p, "matrix")?;
    let cases = f.qa().generate_authz_tests(&matrix);
    let spec = f
        .qa()
        .authz_tests_to_playwright(opt_str(p, "base_url").unwrap_or_default().as_str(), &cases);
    ok(json!({ "cases": cases, "spec": spec }))
}
