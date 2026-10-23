use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures::StreamExt;
use thor::mcp::server::build_mcp_router;
use tower::ServiceExt;

fn message_request(uri: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {}", "01".repeat(32)))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
        .unwrap()
}

#[tokio::test]
async fn authenticated_web_origins_cannot_control_the_browser() {
    let mut request = message_request("/mcp/message?session_id=missing");
    request
        .headers_mut()
        .insert("origin", "https://attacker.test".parse().unwrap());
    let response = build_mcp_router(None, [1; 32])
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unknown_sessions_are_reported_instead_of_silently_accepting_work() {
    let response = build_mcp_router(None, [1; 32])
        .oneshot(message_request("/mcp/message?session_id=missing"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn missing_session_is_an_invalid_request() {
    let response = build_mcp_router(None, [1; 32])
        .oneshot(message_request("/mcp/message"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unauthenticated_requests_are_rejected() {
    let request = Request::builder()
        .uri("/mcp/sse")
        .body(Body::empty())
        .unwrap();
    let response = build_mcp_router(None, [1; 32])
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn streamable_http_returns_json_rpc_results() {
    let response = build_mcp_router(None, [1; 32])
        .oneshot(message_request("/mcp"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["id"], 1);
    assert!(value["result"].is_object());
}

#[tokio::test]
async fn closing_sse_stream_revokes_its_session_endpoint() {
    let router = build_mcp_router(None, [1; 32]);
    let request = Request::builder()
        .uri("/mcp/sse")
        .header("authorization", format!("Bearer {}", "01".repeat(32)))
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let mut stream = response.into_body().into_data_stream();
    let chunk = stream.next().await.unwrap().unwrap();
    let data = std::str::from_utf8(&chunk).unwrap();
    let endpoint = data
        .lines()
        .find_map(|line| line.strip_prefix("data:"))
        .unwrap()
        .trim();
    let accepted = router
        .clone()
        .oneshot(message_request(endpoint))
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    drop(stream);
    let response = router.oneshot(message_request(endpoint)).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn request_body_limit_prevents_unbounded_protocol_inputs() {
    let request = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("authorization", format!("Bearer {}", "01".repeat(32)))
        .header("content-type", "application/json")
        .body(Body::from(format!(
            r#"{{"padding":"{}"}}"#,
            "x".repeat(300_000)
        )))
        .unwrap();
    let response = build_mcp_router(None, [1; 32])
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
