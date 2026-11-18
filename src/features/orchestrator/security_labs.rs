use super::{json, ok, op, opt, req, reqv, str_req, FeatureEngine, Value};

pub(in crate::features) fn oob_listener(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match op(p) {
        "start" => match f.security().start_oob_listener(opt(p, "port").unwrap_or(0)) {
            Ok(port) => ok(json!({ "port": port })),
            Err(e) => Err(e),
        },
        "stop" => {
            f.security().stop_oob_listener();
            ok(json!({ "stopped": true }))
        }
        "token" => ok(json!({ "token": f.security().new_oob_token() })),
        "list" => ok(f.security().oob_interactions(str_req(p, "token")?)),
        _ => ok(json!({ "port": f.security().oob_listener_port() })),
    }
}

pub(in crate::features) fn session_fixation_lab(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    match f.security().session_fixation_lab(&req(p, "probe")?) {
        Ok(r) => ok(r),
        Err(e) => Err(e),
    }
}

pub(in crate::features) fn race_window_repeater(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    if opt::<bool>(p, "live").unwrap_or(false) {
        match f.security().race_window_live(
            str_req(p, "url")?,
            opt(p, "count").unwrap_or(8),
            opt(p, "lab_consent").unwrap_or(false),
        ) {
            Ok(r) => ok(r),
            Err(e) => Err(e),
        }
    } else {
        ok(f.security().race_window_repeater(&reqv(p, "attempts")?))
    }
}

pub(in crate::features) fn ssrf_lab_harness(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    match f
        .security()
        .ssrf_lab_harness(&reqv(p, "targets")?, opt(p, "lab_consent").unwrap_or(false))
    {
        Ok(r) => ok(r),
        Err(e) => Err(e),
    }
}
