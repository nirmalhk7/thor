use super::{json, ok, opt, opt_str, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn authz_matrix(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.security().build_authz_matrix(&reqv(p, "probes")?))
}

pub(in crate::features) fn jwt_deep(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let jwt = str_req(p, "jwt")?;
    match opt_str(p, "secret") {
        Some(secret) => match f.security().verify_jwt_hmac(jwt, &secret) {
            Ok(valid) => ok(json!({ "signature_valid": valid })),
            Err(e) => Err(e),
        },
        None => match f
            .security()
            .inspect_jwt(jwt, opt(p, "now_unix").unwrap_or(0))
        {
            Ok(r) => ok(r),
            Err(e) => Err(e),
        },
    }
}

pub(in crate::features) fn graphql_authz_matrix(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.security().graphql_authz_matrix(&reqv(p, "probes")?))
}

pub(in crate::features) fn websocket_authz_probe(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.security().websocket_authz_probe(&reqv(p, "probes")?))
}
