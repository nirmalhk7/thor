use super::backend;
use super::{ok, opt, opt_str, opt_vec, reqv, FeatureEngine, Value};

pub(in crate::features) fn rate_limit_probe(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if let Some(url) = opt_str(p, "url") {
        match f.backend().rate_limit_probe_http(
            &url,
            opt(p, "count").unwrap_or(5),
            opt(p, "lab_consent").unwrap_or(false),
        ) {
            Ok(r) => ok(r),
            Err(e) => Err(e),
        }
    } else {
        ok(f.backend().rate_limit_probe(&reqv(p, "samples")?))
    }
}

pub(in crate::features) fn pagination_auditor(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if let Some(urls) = opt_vec::<String>(p, "urls") {
        match f.backend().pagination_auditor_http(
            &urls,
            opt(p, "page_size").unwrap_or(20),
            opt(p, "lab_consent").unwrap_or(false),
        ) {
            Ok(r) => ok(r),
            Err(e) => Err(e),
        }
    } else {
        ok(f.backend().pagination_auditor(&reqv(p, "pages")?))
    }
}

pub(in crate::features) fn cache_semantics_lens(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.backend().cache_semantics_lens(&reqv(p, "entries")?))
}

pub(in crate::features) fn open_api_diff_gate(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if let Some(doc) = p.get("openapi").cloned() {
        let paths = backend::BackendEngine::parse_openapi_paths(&doc);
        if let Some(interactions) = opt_vec::<backend::ActualInteraction>(p, "interactions") {
            ok(f.backend().openapi_validate_traffic(&paths, &interactions))
        } else {
            let after = opt_vec(p, "after").unwrap_or(paths.clone());
            let before = opt_vec(p, "before").unwrap_or(paths);
            ok(f.backend().openapi_diff_gate(&before, &after))
        }
    } else if let Some(interactions) = opt_vec::<backend::ActualInteraction>(p, "interactions") {
        ok(f.backend()
            .openapi_validate_traffic(&reqv(p, "paths")?, &interactions))
    } else {
        ok(f.backend()
            .openapi_diff_gate(&reqv(p, "before")?, &reqv(p, "after")?))
    }
}
