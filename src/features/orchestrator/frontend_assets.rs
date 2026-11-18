use super::frontend;
use super::{ok, opt, opt_vec, reqv, FeatureEngine, Value};

pub(in crate::features) fn font_auditor(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.frontend().audit_fonts(
        &reqv(p, "faces")?,
        &opt_vec(p, "used_families").unwrap_or_default(),
    ))
}

pub(in crate::features) fn image_delivery_auditor(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    let from_live = opt::<bool>(p, "from_network").unwrap_or(false)
        || opt::<bool>(p, "from_active_tab").unwrap_or(false);
    let images: Vec<frontend::ImageAsset> = if from_live {
        f.cdp().images_from_network()
    } else if let Some(imgs) = opt_vec(p, "images") {
        imgs
    } else {
        f.cdp().images_from_network()
    };
    ok(f.frontend().image_delivery_auditor(&images))
}
