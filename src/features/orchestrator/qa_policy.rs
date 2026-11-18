use super::{frontend, qa};
use super::{json, ok, opt, opt_str, opt_vec, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn flake_triager(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let test = opt_str(p, "test").unwrap_or_default();
    let results: Vec<bool> = reqv(p, "results")?;
    let radar = f.qa().flake_radar(&test, &results);
    let mut vectors: qa::FlakeVector = opt(p, "vectors").unwrap_or_default();
    let frames = match opt_str(p, "trace_test") {
        Some(t) => f.qa().trace_frames(&t),
        None => opt_vec::<qa::TraceFrame>(p, "frames").unwrap_or_default(),
    };
    let traces: Vec<crate::dx::test_generator::NetworkTrace> =
        opt_vec(p, "traces").unwrap_or_default();
    // Derive signals from frames/traces when the caller omitted a hand-fed vector.
    if vectors.is_empty() {
        vectors = qa::FlakeVector::from_trace_frames(&frames)
            .merge(qa::FlakeVector::from_network_traces(&traces));
    }
    ok(f.qa().triage_flake(&test, &radar, &vectors, &frames))
}

pub(in crate::features) fn quarantine_policy(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    let decision = f.qa().evaluate_quarantine(&req(p, "input")?);
    if !decision.persist_ok {
        return Err(decision
            .persist_err
            .clone()
            .unwrap_or_else(|| "quarantine.yaml persist failed".into()));
    }
    let mut out = serde_json::to_value(&decision).map_err(|e| format!("serialize failed: {e}"))?;
    if let Some(obj) = out.as_object_mut() {
        let filter = f.qa().ci_quarantine_filter();
        let filter = if filter.trim().is_empty() {
            crate::dx::test_generator::TestGenerator::ci_quarantine_filter(&[])
        } else {
            filter
        };
        obj.insert("ci_quarantine_filter".into(), Value::String(filter));
        obj.insert("persist_ok".into(), Value::Bool(decision.persist_ok));
        if let Some(err) = &decision.persist_err {
            obj.insert("persist_err".into(), Value::String(err.clone()));
        }
        obj.insert(
            "decision".into(),
            serde_json::to_value(&decision).unwrap_or_default(),
        );
    }
    Ok(out)
}

pub(in crate::features) fn contract_drift_gate(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa()
        .contract_drift_gate(&reqv(p, "baseline")?, &reqv(p, "live")?))
}

pub(in crate::features) fn hermetic_fixture_pack(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    let pack = f.qa().build_hermetic_fixture_pack(
        str_req(p, "name")?,
        req(p, "identity")?,
        opt_vec(p, "har_mocks").unwrap_or_default(),
        opt_vec(p, "storage").unwrap_or_default(),
    );
    let export = opt_str(p, "export").unwrap_or_default();
    if export == "playwright" {
        ok(json!({
            "pack_digest": pack.pack_digest,
            "playwright_ts": pack.to_playwright(),
            "storage_state_json": pack.storage_state_json(),
        }))
    } else {
        ok(pack)
    }
}

pub(in crate::features) fn a11y_test_gen(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let page_name = opt_str(p, "page_name").unwrap_or_else(|| "active".into());
    let violations: Vec<qa::A11yViolation> = if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let raw = f.evaluate_on_active(frontend::FrontendEngine::a11y_dom_probe_script())?;
        let nodes: Vec<frontend::DomNode> =
            serde_json::from_str(&raw).map_err(|e| format!("invalid a11y DOM probe JSON: {e}"))?;
        let matrix = f.frontend().a11y_matrix(&nodes);
        matrix
            .cells
            .into_iter()
            .filter(|c| c.violated)
            .map(|c| qa::A11yViolation {
                rule_id: c.rule,
                selector: c.node,
                impact: "serious".into(),
                message: c.message,
            })
            .collect()
    } else {
        reqv(p, "violations")?
    };
    ok(f.qa().generate_a11y_spec(&page_name, &violations))
}

pub(in crate::features) fn snapshot_strategy_pack(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    let pixels: Vec<u8> = opt(p, "pixels").unwrap_or_default();
    ok(f.qa().snapshot_strategy_pack(
        str_req(p, "name")?,
        &pixels,
        opt_str(p, "dom").unwrap_or_default().as_str(),
        opt_str(p, "a11y_digest").unwrap_or_default().as_str(),
        opt_str(p, "body").unwrap_or_default().as_str(),
    ))
}
