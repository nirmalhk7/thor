use super::backend;
use super::{
    json, ok, op, opt, opt_str, opt_vec, req, reqv, str_req, AppState, FeatureEngine, Value,
};

pub(in crate::features) fn webhook_sink(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match op(p) {
        "start" => match f.backend().start_webhook_sink(opt(p, "port").unwrap_or(0)) {
            Ok(port) => ok(json!({ "port": port })),
            Err(e) => Err(e),
        },
        "stop" => {
            f.backend().stop_webhook_sink();
            ok(json!({ "stopped": true }))
        }
        "list" => ok(f.backend().webhooks()),
        _ => ok(json!({ "port": f.backend().webhook_sink_port() })),
    }
}

pub(in crate::features) fn idor_matrix(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let from_live = opt::<bool>(p, "from_active_tab").unwrap_or(false)
        || opt::<bool>(p, "from_tamper").unwrap_or(false);
    let request: backend::ApiRequest = if let Some(r) = opt::<backend::ApiRequest>(p, "request") {
        r
    } else if from_live || p.get("request").is_none() {
        let hist = AppState::global()
                        .tamper()
                        .get_latest_request()
                        .ok_or_else(|| {
                            "idor-matrix: no request payload and no tamper history — browse with intercept on first"
                                .to_string()
                        })?;
        backend::ApiRequest {
            method: hist.method,
            url: hist.url,
            headers: hist.headers,
            body: hist.body,
        }
    } else {
        return Err("idor-matrix requires 'request'".into());
    };
    let contexts: Vec<String> = opt_vec(p, "contexts")
        .unwrap_or_else(|| vec!["anonymous".into(), "user".into(), "admin".into()]);
    let owner = opt_str(p, "owner_context");
    ok(f.backend()
        .run_idor_matrix(&request, &contexts, owner.as_deref()))
}

pub(in crate::features) fn trace_adjacency(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.backend().build_trace_graph(&reqv(p, "spans")?))
}

pub(in crate::features) fn mock_stub(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match op(p) {
        "add" => {
            f.backend().add_mock(req(p, "route")?);
            ok(json!({ "added": true }))
        }
        "match" => ok(json!({
            "match": f.backend().match_mock(str_req(p, "method")?, str_req(p, "path")?)
        })),
        _ => {
            let history = AppState::global().tamper().get_history();
            ok(json!({ "mocks_added": f.backend().mock_from_tamper_history(&history) }))
        }
    }
}
