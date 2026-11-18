use super::frontend;
use super::{json, ok, op, opt, opt_str, opt_val, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn state_time_travel(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    match op(p) {
        "record" => ok(json!({
            "index": f.frontend().record_state(str_req(p, "label")?, opt_val(p, "state"))
        })),
        "restore" => ok(json!({
            "state": f.frontend().restore_state(req(p, "index")?)
        })),
        "diff" => ok(f.frontend().state_diff(req(p, "from")?, req(p, "to")?)),
        _ => ok(f.frontend().state_snapshots()),
    }
}

pub(in crate::features) fn component_sandbox(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(json!({
        "html": f.frontend().component_sandbox(
            str_req(p, "tag")?,
            &opt_val(p, "props"),
            opt_str(p, "template_html").unwrap_or_default().as_str(),
        )
    }))
}

pub(in crate::features) fn network_component_binding(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend()
        .bind_network_to_components(&reqv(p, "windows")?, &reqv(p, "events")?))
}

pub(in crate::features) fn focus_order_overlay(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let raw = f.evaluate_on_active(frontend::FrontendEngine::focus_order_probe_script())?;
        let nodes = frontend::FrontendEngine::parse_focus_order_probe(&raw)
            .ok_or_else(|| "invalid focus probe JSON from active tab".to_string())?;
        ok(f.frontend().focus_order_overlay(&nodes))
    } else if let Some(json) = opt_str(p, "probe_json") {
        let nodes = frontend::FrontendEngine::parse_focus_order_probe(&json)
            .ok_or_else(|| "invalid focus probe JSON".to_string())?;
        ok(f.frontend().focus_order_overlay(&nodes))
    } else {
        ok(f.frontend().focus_order_overlay(&reqv(p, "nodes")?))
    }
}

pub(in crate::features) fn event_listener_map(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend().event_listener_map(&reqv(p, "entries")?))
}

pub(in crate::features) fn aria_live_monitor(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend().aria_live_monitor(&reqv(p, "events")?))
}
