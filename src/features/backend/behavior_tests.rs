use super::*;
use serde_json::json;

#[test]
fn trace_keeps_orphan_roots_first_ties_and_unique_service_edges() {
    let spans: Vec<Span> = serde_json::from_value(json!([
        {"span_id":"a","parent_id":"missing","service":"api","name":"root","duration_ms":2.0},
        {"span_id":"b","parent_id":"a","service":"db","name":"first","duration_ms":3.0},
        {"span_id":"c","parent_id":"a","service":"db","name":"second","duration_ms":3.0},
        {"span_id":"d","parent_id":"a","service":"api","name":"local","duration_ms":1.0},
        {"span_id":"e","parent_id":null,"service":"other","name":"tie","duration_ms":5.0}
    ]))
    .unwrap();
    let graph = BackendEngine::new().build_trace_graph(&spans);
    assert_eq!(
        graph.edges,
        BTreeMap::from([("api".into(), vec!["db".into()])])
    );
    assert_eq!(graph.critical_path, ["api:root", "db:first"]);
    assert_eq!(graph.critical_path_ms, 5.0);
}

#[test]
fn trace_nonpositive_roots_leave_critical_path_empty() {
    let span = Span {
        span_id: "a".into(),
        parent_id: None,
        service: "api".into(),
        name: "root".into(),
        duration_ms: 0.0,
    };
    let graph = BackendEngine::new().build_trace_graph(&[span]);
    assert!(graph.critical_path.is_empty());
    assert_eq!(graph.critical_path_ms, 0.0);
}

#[test]
fn database_retains_nonadjacent_repeated_tables_and_neighbor_order() {
    let report = BackendEngine::new().analyze_db(&[
        "SELECT * FROM Users JOIN Orders JOIN Users JOIN Cache".into(),
        "SELECT * FROM select JOIN ;".into(),
    ]);
    assert_eq!(report.total_queries, 2);
    assert_eq!(report.adjacency["users"], ["orders", "users", "cache"]);
    assert_eq!(report.adjacency["orders"], ["users", "cache"]);
    assert_eq!(report.adjacency["cache"], ["users", "orders"]);
    assert!(report.n_plus_one.is_empty());
    assert_eq!(
        database::extract_tables("UPDATE [雪] SET x=1 FROM users JOIN users INTO orders"),
        ["雪", "users", "orders"]
    );
}

#[test]
fn schema_orders_required_before_children_and_stops_on_type_mismatch() {
    let engine = BackendEngine::new();
    let schema = json!({"required":["missing",17,"missing"],"properties":{"a":{"type":"integer"},"b":{"items":{"type":"string"}}}});
    assert_eq!(
        engine.validate_contract(&json!({"a":1.5,"b":[true,"ok",null]}), &schema),
        [
            "$: missing required property 'missing'",
            "$: missing required property 'missing'",
            "$.a: expected integer, got number",
            "$.b[0]: expected string, got boolean",
            "$.b[2]: expected string, got null",
        ]
    );
    assert_eq!(
        engine.validate_contract(&json!({}), &json!({"type":"unknown","required":["x"]})),
        ["$: expected unknown, got object"]
    );
    assert!(engine
        .validate_contract(&json!(u64::MAX), &json!({"type":"integer"}))
        .is_empty());
}

fn get(port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn route(body: &str) -> Vec<StubRouteSpec> {
    vec![StubRouteSpec {
        method: "GET".into(),
        path: "/value".into(),
        status: 201,
        body: body.into(),
    }]
}

#[test]
fn stub_rejection_preserves_listener_bind_failure_clears_it_and_restart_resets_hits() {
    let engine = BackendEngine::new();
    let port = engine
        .start_live_stub_server(0, route("first"), None)
        .unwrap();
    assert!(get(port, "/value").ends_with("\r\n\r\nfirst"));
    assert_eq!(engine.live_stub_server(&[]).hit_count, 1);
    assert_eq!(
        engine.start_live_stub_server(0, route("bad"), Some("https://example.com")),
        Err("live stub refuses non-local advertise URL: https://example.com".into())
    );
    assert_eq!(engine.live_stub_server_port(), Some(port));
    assert!(get(port, "/missing").ends_with("{\"error\":\"not found\"}"));
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    assert!(engine
        .start_live_stub_server(occupied.local_addr().unwrap().port(), route("bad"), None)
        .unwrap_err()
        .starts_with("bind failed: "));
    assert_eq!(engine.live_stub_server_port(), None);
    let port = engine
        .start_live_stub_server(0, route("second"), None)
        .unwrap();
    assert_eq!(engine.live_stub_server(&[]).hit_count, 0);
    assert!(get(port, "/value").ends_with("\r\n\r\nsecond"));
    let report = engine.live_stub_server(&[]);
    assert_eq!(report.hit_count, 1);
    assert_eq!(report.routes, 0);
    engine.stop_live_stub_server();
    assert_eq!(engine.live_stub_server_port(), None);
}

#[test]
fn sse_restart_replaces_events_and_resets_delivery_count() {
    let engine = BackendEngine::new();
    let events = vec![StreamLabEvent {
        kind: "update".into(),
        payload: "雪".into(),
        timestamp_ms: 1,
    }];
    let port = engine.start_sse_ws_server_lab(0, events, None).unwrap();
    assert!(get(port, "/events").ends_with("retry: 1000\n\nevent: update\ndata: 雪\n\n"));
    assert_eq!(engine.sse_ws_server_lab(&[]).live_event_count, 1);
    assert!(engine
        .start_sse_ws_server_lab(0, vec![], Some("https://example.com"))
        .is_err());
    assert_eq!(engine.sse_ws_server_lab_port(), Some(port));
    let port = engine.start_sse_ws_server_lab(0, vec![], None).unwrap();
    assert_eq!(engine.sse_ws_server_lab(&[]).live_event_count, 0);
    assert!(get(port, "/events").ends_with("retry: 1000\n\n"));
    engine.stop_sse_ws_server_lab();
    assert_eq!(engine.sse_ws_server_lab_port(), None);
}

#[test]
fn webhook_and_oob_bind_failures_clear_previous_listener_slots() {
    let engine = BackendEngine::new();
    let security = crate::features::security::SecurityEngine::new();
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let blocked_port = occupied.local_addr().unwrap().port();
    let port = engine.start_webhook_sink(0).unwrap();
    assert!(get(port, "/quality-characterization").ends_with("\r\n\r\nok"));
    assert!(AppState::global()
        .features()
        .backend()
        .webhooks_matching("/quality-characterization")
        .iter()
        .any(|event| event.method == "GET"));
    assert!(engine.start_webhook_sink(blocked_port).is_err());
    assert_eq!(engine.webhook_sink_port(), None);
    let port = security.start_oob_listener(0).unwrap();
    assert!(get(port, "/quality-characterization").ends_with("\r\n\r\nok"));
    assert!(security.start_oob_listener(blocked_port).is_err());
    assert_eq!(security.oob_listener_port(), None);
}
