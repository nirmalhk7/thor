use super::*;

#[test]
fn required_payload_errors_keep_parameter_order_and_string_distinction() {
    let f = FeatureEngine::new();
    assert_eq!(
        FeatureOrchestrator::dispatch(&f, FeatureId::ContractValidation, &json!({})).unwrap_err(),
        "missing required param: value"
    );
    assert_eq!(
        FeatureOrchestrator::dispatch(&f, FeatureId::ContractValidation, &json!({"value":null}))
            .unwrap_err(),
        "missing required param: schema"
    );
    assert_eq!(
        FeatureOrchestrator::dispatch(&f, FeatureId::InjectionScanner, &json!({"input":null}))
            .unwrap_err(),
        "missing required string param: input"
    );
    let result = FeatureOrchestrator::dispatch(
        &f,
        FeatureId::ContractValidation,
        &json!({"value":null,"schema":{"type":"null"}}),
    )
    .unwrap();
    assert_eq!(result, json!({"errors":[]}));
}

#[test]
fn local_labs_preserve_advertise_rejection_port_reports_and_stop_envelopes() {
    let f = FeatureEngine::new();
    for (id, payload_key) in [
        (FeatureId::SseWsServerLab, "events"),
        (FeatureId::LiveStubServer, "routes"),
    ] {
        let rejected = FeatureOrchestrator::dispatch(
            &f,
            id,
            &json!({"op":"start","advertise_url":"https://example.invalid/"}),
        );
        assert!(rejected
            .unwrap_err()
            .contains("refuses non-local advertise URL"));
        let mut payload =
            json!({"op":"start","port":"malformed","advertise_url":"http://127.0.0.1/"});
        payload[payload_key] = json!("malformed");
        let started = FeatureOrchestrator::dispatch(&f, id, &payload).unwrap();
        assert!(started["port"].as_u64().unwrap() > 0);
        let report = FeatureOrchestrator::dispatch(&f, id, &json!({})).unwrap();
        assert_eq!(report["bound_port"], started["port"]);
        assert_eq!(report["live"], true);
        assert_eq!(
            FeatureOrchestrator::dispatch(&f, id, &json!({"op":"stop"})).unwrap(),
            json!({"stopped":true})
        );
        assert_eq!(
            FeatureOrchestrator::dispatch(&f, id, &json!({})).unwrap()["live"],
            false
        );
    }
}

#[test]
fn public_ssrf_target_requires_consent_before_any_request() {
    let f = FeatureEngine::new();
    let payload = json!({"targets":[{"url":"https://example.invalid/","resolved_ip":null}],"lab_consent":false});
    assert_eq!(
        FeatureOrchestrator::dispatch(&f, FeatureId::SsrfLabHarness, &payload).unwrap_err(),
        "SSRF lab refused: public target without lab_consent: https://example.invalid/"
    );
}
