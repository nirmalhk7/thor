use super::*;
use serde_json::json;

#[test]
fn tool_results_keep_existing_id_fallback_fragment_and_timestamp_rules() {
    let stream = concat!(
        "data: {\"created_ms\":100,\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":4,\"id\":\"first\",\"function\":{\"name\":\"search\",\"arguments\":\"{\"}}]}}]}\n",
        "data: {\"timestamp_ms\":140,\"type\":\"tool_result\",\"tool_call_id\":\"unknown\",\"is_error\":true}\n",
        "data: {\"created_ms\":160,\"choices\":[{\"message\":{\"tool_calls\":[{\"index\":4,\"id\":\"new\",\"function\":{\"name\":\"\",\"arguments\":\"}\"}}]}}]}\n",
        "data: {\"timestamp_ms\":90,\"role\":\"tool\",\"tool_call_id\":\"first\"}\n",
    );
    let events = AiEngine::tool_call_events_from_sse(stream);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].name, "search");
    assert_eq!(events[0].arguments_json, "{}");
    assert!(events[0].success);
    assert_eq!(events[0].latency_ms, 40);
    assert!(events[0].error.is_none());
}

#[test]
fn schema_errors_keep_required_property_and_additional_property_order() {
    let report = AiEngine::new().json_schema_guard(
        &json!({"z":false,"a":1}),
        &json!({"type":"object","required":["missing","missing",7],"properties":{"a":{"type":"string"}},"additionalProperties":false}),
    );
    assert_eq!(
        report.errors,
        [
            "$: missing required 'missing'",
            "$: missing required 'missing'",
            "$.a: expected string",
            "$: additional property 'z' not allowed",
        ]
    );
    let engine = AiEngine::new();
    assert!(
        engine
            .json_schema_guard(&json!(1.5), &json!({"type":"integer"}))
            .ok
    );
    assert!(
        engine
            .json_schema_guard(&json!(null), &json!({"type":"unrecognized"}))
            .ok
    );
    assert!(engine.json_schema_guard(&json!(null), &json!(false)).ok);
}

#[test]
fn schema_depth_limit_stops_recursive_arrays() {
    let mut value = json!("leaf");
    let mut schema = json!({"type":"string"});
    for _ in 0..40 {
        value = json!([value]);
        schema = json!({"type":"array","items":schema});
    }
    let report = AiEngine::new().json_schema_guard(&value, &schema);
    assert_eq!(
        report.errors,
        [format!(
            "${}: exceeded max schema depth 32",
            "[0]".repeat(33)
        )]
    );
}

#[test]
fn fanout_preserves_endpoint_order_auth_payload_usage_and_mock_replay() {
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::{Json, Router};
    use std::sync::mpsc;
    use std::time::Duration;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let (sent, received) = mpsc::channel();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let app = Router::new().fallback(move |request: Request<Body>| {
                let sent = sent.clone();
                async move {
                    let (parts, body) = request.into_parts();
                    let body: serde_json::Value = serde_json::from_slice(&to_bytes(body, 16_384).await.unwrap()).unwrap();
                    let path = parts.uri.path().to_owned();
                    sent.send((path.clone(), parts.method, parts.headers, body)).unwrap();
                    Json(if path == "/openai" {
                        json!({"choices":[{"message":{"content":"from-openai"}}],"usage":{"completion_tokens":7}})
                    } else {
                        json!({"content":[{"text":"reply"}]})
                    })
                }
            });
            axum::serve(listener, app).with_graceful_shutdown(async { let _ = stopped.await; }).await.unwrap();
        });
    });
    let body = json!({"messages":[{"role":"user","content":"hello"}]});
    let engine = AiEngine::new();
    engine.record_mock(&mock_signature("offline", &body), "cached");
    let endpoints = [
        ModelEndpoint {
            model: "openai".into(),
            url: format!("http://{address}/openai"),
            api_key: Some("local-test".into()),
            body: body.clone(),
        },
        ModelEndpoint {
            model: "offline".into(),
            url: "http://127.0.0.1:0/never".into(),
            api_key: None,
            body: body.clone(),
        },
        ModelEndpoint {
            model: "anthropic".into(),
            url: format!("http://{address}/anthropic"),
            api_key: None,
            body: body.clone(),
        },
    ];
    let runs = engine.compare_models_fanout(&endpoints);
    assert_eq!(
        runs.iter().map(|r| r.model.as_str()).collect::<Vec<_>>(),
        ["openai", "offline", "anthropic"]
    );
    assert_eq!(
        runs.iter().map(|r| r.output.as_str()).collect::<Vec<_>>(),
        ["from-openai", "cached", "reply"]
    );
    assert_eq!(
        runs.iter().map(|r| r.prompt_tokens).collect::<Vec<_>>(),
        [2, 2, 2]
    );
    assert_eq!(
        runs.iter().map(|r| r.completion_tokens).collect::<Vec<_>>(),
        [7, 2, 2]
    );
    assert_eq!(runs[1].latency_ms, 0.0);
    for _ in 0..2 {
        let (path, method, headers, posted) =
            received.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(posted, body);
        if path == "/openai" {
            assert_eq!(headers["authorization"], "Bearer local-test");
        } else {
            assert_eq!(path, "/anthropic");
            assert!(headers.get("authorization").is_none());
        }
    }
    assert!(received.try_recv().is_err());
    stop.send(()).unwrap();
    server.join().unwrap();
}
