use super::backend;
use super::{json, ok, op, opt_str, opt_vec, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn grpc_graphql(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if let Some(query) = opt_str(p, "query") {
        ok(f.backend().analyze_graphql(&query))
    } else if let Some(path) = opt_str(p, "path") {
        ok(json!({ "grpc": f.backend().parse_grpc_path(&path) }))
    } else {
        Err("provide 'query' or 'path'".into())
    }
}

pub(in crate::features) fn chaos_injection(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match op(p) {
        "add" => {
            f.backend().add_chaos_rule(req(p, "rule")?);
            ok(json!({ "added": true }))
        }
        _ => ok(f.backend().apply_chaos(
            str_req(p, "url")?,
            opt_str(p, "request_id").unwrap_or_default().as_str(),
        )),
    }
}

pub(in crate::features) fn idempotency_auditor(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.backend().audit_idempotency(&reqv(p, "events")?))
}

pub(in crate::features) fn db_adjacency(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let queries: Vec<String> = req(p, "queries")?;
    match opt_vec::<backend::CacheOp>(p, "cache_ops") {
        Some(ops) => ok(f.backend().analyze_db_cache(&queries, &ops)),
        None => ok(f.backend().analyze_db(&queries)),
    }
}

pub(in crate::features) fn webhook_signature_vault(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.backend().webhook_signature_vault(&req(p, "input")?))
}
