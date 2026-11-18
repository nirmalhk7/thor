use super::ai;
use super::{ok, opt, opt_str, opt_vec, reqv, FeatureEngine, Value};

pub(in crate::features) fn eval_replay(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if let Some(session) = opt::<crate::dx::test_generator::RecordedSession>(p, "session") {
        ok(f.ai().eval_from_session(&session))
    } else {
        ok(f.ai().eval_replay(&reqv(p, "cases")?))
    }
}

pub(in crate::features) fn model_comparison(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match opt_vec::<ai::ModelEndpoint>(p, "endpoints") {
        Some(eps) => ok(f.ai().compare_models_fanout(&eps)),
        None => ok(f.ai().compare_models(&reqv(p, "runs")?)),
    }
}

pub(in crate::features) fn agent_trace(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if let Some(sse) = opt_str(p, "sse") {
        let steps = ai::AiEngine::agent_steps_from_sse(&sse);
        ok(f.ai().analyze_agent_trace(&steps))
    } else {
        ok(f.ai().analyze_agent_trace(&reqv(p, "steps")?))
    }
}

pub(in crate::features) fn llm_load(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.ai().llm_load_report(&reqv(p, "stats")?))
}

pub(in crate::features) fn deterministic_replay(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.ai()
        .deterministic_replay(&reqv(p, "outputs")?, opt(p, "temperature").unwrap_or(0.0)))
}
