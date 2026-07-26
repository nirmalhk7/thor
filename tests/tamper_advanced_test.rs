use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use thor::net::tamper::{InterceptedRequest, TamperRule, TrafficTamperEngine};

#[test]
fn test_overlapping_rules() {
    let engine = TrafficTamperEngine::new();
    
    // Add two rules that match the same URL but different conditions or same header
    engine.add_rule(TamperRule {
        id: "rule_1".to_string(),
        enabled: true,
        target_url_pattern: "api.example.com".to_string(),
        match_header_key: None,
        replace_header_key: "X-Overlapping".to_string(),
        replace_header_value: "value1".to_string(),
    });

    engine.add_rule(TamperRule {
        id: "rule_2".to_string(),
        enabled: true,
        target_url_pattern: "api.example.com".to_string(),
        match_header_key: None,
        replace_header_key: "X-Overlapping".to_string(),
        replace_header_value: "value2".to_string(),
    });

    let req = InterceptedRequest {
        id: "req_overlap".to_string(),
        timestamp: 1000,
        method: "GET".to_string(),
        url: "https://api.example.com/data".to_string(),
        headers: HashMap::new(),
        body: None,
        is_modified: false,
    };

    let processed = engine.process_outgoing_request(req).unwrap();
    // Rule 2 should overwrite Rule 1 because it was added later and we iterate in order
    assert_eq!(processed.headers.get("X-Overlapping").unwrap(), "value2");
}

#[test]
fn test_recursive_drop_requests() {
    let engine = TrafficTamperEngine::new();
    engine.set_interception_enabled(true);

    let req = InterceptedRequest {
        id: "req_drop_recurse".to_string(),
        timestamp: 1000,
        method: "GET".to_string(),
        url: "https://api.example.com/data".to_string(),
        headers: HashMap::new(),
        body: None,
        is_modified: false,
    };

    engine.process_outgoing_request(req);
    assert_eq!(engine.get_pending_requests().len(), 1);

    // Drop first time
    assert!(engine.drop_request("req_drop_recurse"));
    // Drop second time - should be false
    assert!(!engine.drop_request("req_drop_recurse"));
}

#[test]
fn test_interceptor_toggle_under_load() {
    let engine = Arc::new(TrafficTamperEngine::new());
    let mut handles = vec![];

    let engine_clone = Arc::clone(&engine);
    let toggle_handle = thread::spawn(move || {
        for i in 0..1000 {
            engine_clone.set_interception_enabled(i % 2 == 0);
        }
    });

    for i in 0..10 {
        let engine_clone = Arc::clone(&engine);
        let handle = thread::spawn(move || {
            for j in 0..100 {
                let req = InterceptedRequest {
                    id: format!("req_{}_{}", i, j),
                    timestamp: 1000,
                    method: "GET".to_string(),
                    url: "https://api.example.com/data".to_string(),
                    headers: HashMap::new(),
                    body: None,
                    is_modified: false,
                };
                engine_clone.process_outgoing_request(req);
            }
        });
        handles.push(handle);
    }

    toggle_handle.join().unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
}
