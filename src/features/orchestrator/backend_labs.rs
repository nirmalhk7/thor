use super::backend::BackendEngine;
use super::{local_lab, ok, opt, opt_str, req, reqv, FeatureEngine, LocalLab, Value};

pub(in crate::features) fn multipart_workbench(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.backend().multipart_workbench(&reqv(p, "parts")?))
}

pub(in crate::features) fn sse_ws_server_lab(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    local_lab(
        f.backend(),
        p,
        LocalLab::new(
            "events",
            BackendEngine::start_sse_ws_server_lab,
            BackendEngine::stop_sse_ws_server_lab,
            BackendEngine::sse_ws_server_lab,
        ),
    )
}

pub(in crate::features) fn live_stub_server(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    local_lab(
        f.backend(),
        p,
        LocalLab::new(
            "routes",
            BackendEngine::start_live_stub_server,
            BackendEngine::stop_live_stub_server,
            BackendEngine::live_stub_server,
        ),
    )
}

pub(in crate::features) fn header_contract_fuzzer(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    let base = req(p, "base")?;
    let contract = req(p, "contract")?;
    let execute = opt::<bool>(p, "execute").unwrap_or(false);
    if execute {
        match f.backend().header_contract_fuzzer_gated(
            &base,
            &contract,
            opt_str(p, "url").as_deref(),
            true,
            opt(p, "lab_consent").unwrap_or(false),
        ) {
            Ok(r) => ok(r),
            Err(e) => Err(e),
        }
    } else {
        ok(f.backend().header_contract_fuzzer(&base, &contract))
    }
}
