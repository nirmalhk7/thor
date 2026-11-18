use super::{ok, opt, req, reqv, FeatureEngine, Value};

pub(in crate::features) fn coverage_adjacency_map(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa()
        .coverage_adjacency(&reqv(p, "observed")?, &reqv(p, "covered")?))
}

pub(in crate::features) fn parallel_shard_planner(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa()
        .plan_parallel_shards(&reqv(p, "scenarios")?, opt(p, "shard_count").unwrap_or(2)))
}

pub(in crate::features) fn trace_property_miner(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa().mine_trace_properties(&reqv(p, "traces")?))
}

pub(in crate::features) fn runtime_self_heal(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa()
        .runtime_self_heal(&req(p, "signals")?, &reqv(p, "live")?))
}

pub(in crate::features) fn oracle_synthesizer(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa().synthesize_oracles(&req(p, "observation")?))
}

pub(in crate::features) fn mutation_hook_bridge(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.qa().mutation_hook_bridge(&reqv(p, "steps")?))
}
