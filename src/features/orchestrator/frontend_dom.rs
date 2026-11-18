use super::frontend;
use super::{json, ok, opt, opt_str, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn component_tree_lens(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let react_raw = f
            .evaluate_on_active(frontend::FrontendEngine::react_devtools_probe_script())
            .unwrap_or_else(|_| "{\"present\":false}".into());
        let raw = f.evaluate_on_active(frontend::FrontendEngine::a11y_dom_probe_script())?;
        let nodes: Vec<frontend::DomNode> =
            serde_json::from_str(&raw).map_err(|e| format!("invalid DOM probe JSON: {e}"))?;
        let root = nodes.into_iter().next().unwrap_or_default();
        let tree = f.frontend().build_component_tree(&root);
        let mut out = serde_json::to_value(&tree).map_err(|e| format!("serialize tree: {e}"))?;
        if let Some(obj) = out.as_object_mut() {
            let react: Value = serde_json::from_str(&react_raw).unwrap_or(json!({}));
            obj.insert("react_devtools".into(), react);
        }
        Ok(out)
    } else {
        ok(f.frontend().build_component_tree(&req(p, "root")?))
    }
}

pub(in crate::features) fn a11y_lens(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let raw = f.evaluate_on_active(frontend::FrontendEngine::a11y_dom_probe_script())?;
        let nodes: Vec<frontend::DomNode> =
            serde_json::from_str(&raw).map_err(|e| format!("invalid a11y DOM probe JSON: {e}"))?;
        ok(f.frontend().a11y_matrix(&nodes))
    } else {
        ok(f.frontend().a11y_matrix(&reqv(p, "nodes")?))
    }
}

pub(in crate::features) fn z_index_visualizer(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend().visualize_zindex(&reqv(p, "elements")?))
}

pub(in crate::features) fn hydration_diff_lens(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend()
        .hydration_diff_lens(str_req(p, "server_html")?, str_req(p, "client_html")?))
}

pub(in crate::features) fn dom_snapshot_capture(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let raw = f.evaluate_on_active(
            crate::features::cdp_bridge::CdpBridge::dom_snapshot_capture_script(),
        )?;
        let root = crate::features::cdp_bridge::CdpBridge::parse_dom_snapshot(&raw)
            .ok_or_else(|| "invalid DOM snapshot JSON from active tab".to_string())?;
        ok(f.frontend().dom_snapshot_capture(&root))
    } else if let Some(json) = opt_str(p, "json").or_else(|| opt_str(p, "dom_json")) {
        let root = crate::features::cdp_bridge::CdpBridge::parse_dom_snapshot(&json)
            .ok_or_else(|| "invalid DOM snapshot JSON".to_string())?;
        ok(f.frontend().dom_snapshot_capture(&root))
    } else {
        ok(f.frontend().dom_snapshot_capture(&req(p, "root")?))
    }
}
