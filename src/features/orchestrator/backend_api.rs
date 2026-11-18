use super::backend;
use super::{json, ok, op, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn api_workbench(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let request: backend::ApiRequest = req(p, "request")?;
    match f.backend().send_blocking(&request) {
        Ok(resp) => ok(resp),
        Err(e) => Err(e),
    }
}

pub(in crate::features) fn auth_vault(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match op(p) {
        "store" => {
            let name = str_req(p, "name")?;
            let scheme: backend::AuthScheme = req(p, "scheme")?;
            f.backend().store_auth(name, scheme);
            ok(json!({ "contexts": f.backend().auth_contexts() }))
        }
        _ => ok(json!({ "contexts": f.backend().auth_contexts() })),
    }
}

pub(in crate::features) fn contract_validation(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(json!({
        "errors": f
            .backend()
            .validate_contract(&req::<Value>(p, "value")?, &req::<Value>(p, "schema")?)
    }))
}

pub(in crate::features) fn request_chaining(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match f.backend().run_chain(&reqv(p, "steps")?) {
        Ok(r) => ok(r),
        Err(e) => Err(e),
    }
}
