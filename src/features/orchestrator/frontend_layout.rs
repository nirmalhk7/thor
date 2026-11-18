use super::frontend;
use super::{ok, opt, reqv, FeatureEngine, Value};

pub(in crate::features) fn responsive_matrix(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend().responsive_matrix(&reqv(p, "captures")?))
}

pub(in crate::features) fn cls_heatmap(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let raw = f.evaluate_on_active(frontend::FrontendEngine::cls_entries_probe_script())?;
        let entries: Vec<frontend::LayoutShiftEntry> =
            serde_json::from_str(&raw).map_err(|e| format!("invalid CLS probe JSON: {e}"))?;
        ok(f.frontend().cls_heatmap(&entries))
    } else {
        ok(f.frontend().cls_heatmap(&reqv(p, "entries")?))
    }
}

pub(in crate::features) fn layout_thrash_detector(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.frontend().layout_thrash_detector(&reqv(p, "ops")?))
}

pub(in crate::features) fn media_cq_matrix(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.frontend().media_cq_matrix(&reqv(p, "captures")?))
}
