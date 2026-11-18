use super::{ok, reqv, FeatureEngine, Value};

pub(in crate::features) fn oauth_flow_capture(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.backend().oauth_flow_capture(&reqv(p, "steps")?))
}

pub(in crate::features) fn queue_adjacency(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.backend().queue_adjacency(&reqv(p, "hops")?))
}

pub(in crate::features) fn grpc_stream_workbench(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.backend().grpc_stream_workbench(&reqv(p, "frames")?))
}
