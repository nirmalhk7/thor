use super::{ok, opt, req, reqv, FeatureEngine, Value};

pub(in crate::features) fn json_schema_guard(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().json_schema_guard(
        &req::<serde_json::Value>(p, "value")?,
        &req::<serde_json::Value>(p, "schema")?,
    ))
}

pub(in crate::features) fn hallucination_oracle(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().hallucination_oracle(&reqv(p, "checks")?))
}

pub(in crate::features) fn embedding_rag_bench(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai()
        .embedding_rag_bench(&reqv(p, "pairs")?, opt(p, "k").unwrap_or(5)))
}

pub(in crate::features) fn eval_dataset_miner(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai().eval_dataset_miner(&reqv(p, "cases")?))
}
