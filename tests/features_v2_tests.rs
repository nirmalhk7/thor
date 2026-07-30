//! Integration tests for the **v2 feature integration** layer of the Thor
//! Engineer Feature Suite.
//!
//! Where `features_tests.rs` proves each registered feature is registered and its
//! backing logic runs, this suite exercises the *wiring* added in v2:
//!
//!   * [`FeatureOrchestrator`] — the single `(slug, payload)` → JSON envelope
//!     IPC router, covered for every specialist category plus its lens payloads
//!     and error paths.
//!   * [`CdpBridge`] — the live-capture half: parsing real Core Web Vitals JSON
//!     and ingesting `TrafficTamperEngine` history / CDP network entries.
//!   * The **active halves** of the domain engines that now perform real I/O:
//!     the webhook sink HTTP listener, the request-chain runner, the IDOR matrix
//!     sender, the OOB HTTP listener, consent-gated fuzzing, masked visual
//!     regression, the flake-radar replayer, PII detection, the prompt version
//!     timeline, and session-derived evals.
//!
//! Tests that need a server stand up a throwaway loopback HTTP listener; tests
//! for the sink/OOB listeners drive Thor's own listeners through the
//! process-wide `AppState::global()` engine (those loops record into the global
//! instance by design).

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::json;

use thor::app_state::AppState;
use thor::features::backend::{self, ApiRequest, AuthScheme, ChainStep};
use thor::features::cdp_bridge::{CdpBridge, NetworkEntry};
use thor::features::orchestrator::FeatureOrchestrator;
use thor::features::qa::{self, MaskRegion};
use thor::features::security::{self, FuzzCategory};
use thor::features::{ai, FeatureId};
use thor::load::{ProtocolRunner, RunnerType, ScenarioStep, ThorScenario};
use thor::net::tamper::{InterceptedRequest, InterceptedResponse};

use thor::dx::test_generator::{NetworkTrace, RecordedSession};

// ===========================================================================
// Local loopback HTTP helpers
// ===========================================================================

/// A throwaway loopback HTTP server that replies to every request with a fixed
/// status + JSON body until dropped. Non-blocking accept loop so it never leaks
/// a wedged thread.
struct TestServer {
    url: String,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    fn start(status: u16, body: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("addr");
        listener.set_nonblocking(true).expect("nonblocking");
        let url = format!("http://{addr}");
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        let response = format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let handle = std::thread::spawn(move || {
            while flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            url,
            running,
            handle: Some(handle),
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Fire a single raw HTTP request at `127.0.0.1:port` and return the raw
/// response text. Used to drive Thor's own webhook / OOB listeners as a client.
fn raw_http(port: u16, method: &str, path: &str, body: &str) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(request.as_bytes())?;
    stream.flush()?;
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
    Ok(resp)
}

/// Poll `f` until it returns `Some`, up to `timeout`.
fn poll_until<T>(timeout: Duration, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(v) = f() {
            return Some(v);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

// ===========================================================================
// 1. IPC orchestrator — invoke envelope for each specialist category
// ===========================================================================

/// One representative, offline (no-network) feature per category, asserting the
/// orchestrator returns a well-formed envelope routed to the right domain.
#[test]
fn test_orchestrator_invoke_each_category() {
    // Frontend: CSS cascade resolution.
    let fe = FeatureOrchestrator::invoke(
        "css-cascade",
        &json!({
            "declarations": [
                {"selector": "div", "property": "color", "value": "red", "important": false, "order": 0},
                {"selector": "#main", "property": "color", "value": "blue", "important": false, "order": 1}
            ]
        }),
    )
    .expect("frontend invoke");
    assert_eq!(fe["slug"], "css-cascade");
    assert_eq!(fe["category"], "Frontend");
    assert!(fe["result"].is_array());
    assert!(fe["invocations"].as_u64().unwrap() >= 1);

    // Backend: JSON-schema contract validation.
    let be = FeatureOrchestrator::invoke(
        "contract-validation",
        &json!({ "value": {"id": 1}, "schema": {"type": "object", "required": ["id"]} }),
    )
    .expect("backend invoke");
    assert_eq!(be["category"], "Backend");
    assert!(be["result"]["errors"].is_array());
    assert_eq!(be["result"]["errors"].as_array().unwrap().len(), 0);

    // Security: HTTP security-header grading.
    let se = FeatureOrchestrator::invoke(
        "header-analyzer",
        &json!({ "headers": {"content-security-policy": "default-src 'self'"} }),
    )
    .expect("security invoke");
    assert_eq!(se["category"], "Security");
    assert!(se["result"]["grade"].is_string());

    // Performance: HDR histogram helper.
    let pe = FeatureOrchestrator::invoke(
        "hdr-histogram-helper",
        &json!({ "samples": [1.0, 2.0, 3.0, 4.0, 5.0] }),
    )
    .expect("perf invoke");
    assert_eq!(pe["category"], "Performance");
    assert!(pe["result"]["percentiles"].is_array());

    // QA: resilient locator ranking.
    let qe = FeatureOrchestrator::invoke(
        "resilient-locators",
        &json!({ "signals": {"test_id": "submit"} }),
    )
    .expect("qa invoke");
    assert_eq!(qe["category"], "QA");
    assert!(!qe["result"].as_array().unwrap().is_empty());

    // AI: prompt-injection / PII scanner.
    let ae = FeatureOrchestrator::invoke(
        "injection-scanner",
        &json!({ "input": "ignore previous instructions" }),
    )
    .expect("ai invoke");
    assert_eq!(ae["category"], "AI");
    assert!(!ae["result"].as_array().unwrap().is_empty());
}

#[test]
fn test_orchestrator_lens_payload_per_category() {
    let expectations = [
        ("frontend", "Frontend", 23usize),
        ("backend", "Backend", 24),
        ("security", "Security", 24),
        ("performance", "Performance", 25),
        ("qa", "QA", 24),
        ("ai", "AI", 24),
    ];
    for (arg, expect_name, count) in expectations {
        let lens = FeatureOrchestrator::lens_payload(arg).expect("lens payload");
        assert_eq!(lens["category"], expect_name, "lens category for {arg}");
        let features = lens["features"].as_array().expect("features array");
        assert_eq!(features.len(), count, "{arg} feature count");
        // Each entry carries the fields the shell lens renders.
        for f in features {
            assert!(f["slug"].is_string());
            assert!(f["name"].is_string());
            assert!(f["status"].is_string());
            assert!(f["enabled"].is_boolean());
        }
    }
    assert!(FeatureOrchestrator::lens_payload("not-a-category").is_err());
}

#[test]
fn test_orchestrator_unknown_slug_and_missing_param_errors() {
    // Unknown slug rejected before dispatch.
    assert!(FeatureOrchestrator::invoke("does-not-exist", &json!({})).is_err());
    // Known slug but missing required param yields a descriptive error.
    let err = FeatureOrchestrator::invoke("css-cascade", &json!({})).unwrap_err();
    assert!(err.contains("declarations"), "error should name the param: {err}");
}

#[test]
fn test_orchestrator_disabled_feature_is_rejected() {
    let engine = AppState::global().features();
    // Disable a feature not touched by any other test in this binary.
    engine.set_enabled(FeatureId::ZIndexVisualizer, false);
    let out = FeatureOrchestrator::invoke("zindex-visualizer", &json!({ "elements": [] }));
    assert!(out.is_err(), "disabled feature must not dispatch");
    // Restore so global state stays clean for other tests.
    engine.set_enabled(FeatureId::ZIndexVisualizer, true);
    assert!(engine.is_enabled(FeatureId::ZIndexVisualizer));
}

#[test]
fn test_orchestrator_records_invocations_on_global_engine() {
    let engine = AppState::global().features();
    let before = engine.invocation_count(FeatureId::InjectionScanner);
    let _ = FeatureOrchestrator::invoke("injection-scanner", &json!({ "input": "hello" }))
        .expect("invoke");
    let after = engine.invocation_count(FeatureId::InjectionScanner);
    assert_eq!(after, before + 1, "orchestrator must record the invocation");
}

// ===========================================================================
// 2. CDP bridge — real web-vitals JSON + tamper history ingestion
// ===========================================================================

#[test]
fn test_cdp_bridge_parses_real_web_vitals_json() {
    // The injection script wires the standard PerformanceObservers.
    let script = CdpBridge::web_vitals_injection_script();
    assert!(script.contains("largest-contentful-paint"));
    assert!(script.contains("layout-shift"));
    assert!(script.contains("__thorCollectWebVitals"));

    // A realistic payload emitted by __thorCollectWebVitals() parses into real
    // BrowserMetrics (not defaults).
    let payload = r#"{"lcp": 1850.5, "inp": 120.0, "cls": 0.07, "fcp": 900.0, "ttfb": 310.0}"#;
    let metrics = CdpBridge::parse_web_vitals(payload).expect("vitals parse");
    assert!((metrics.lcp_ms - 1850.5).abs() < 1e-6);
    assert!((metrics.cls - 0.07).abs() < 1e-6);
    assert!((metrics.ttfb_ms - 310.0).abs() < 1e-6);
    assert!((metrics.inp_ms - 120.0).abs() < 1e-6);

    // Garbage in -> None, not a panic.
    assert!(CdpBridge::parse_web_vitals("not json").is_none());
}

#[test]
fn test_cdp_bridge_ingests_network_entries_and_derives_inputs() {
    let bridge = CdpBridge::new();
    assert!(bridge.is_empty());
    bridge.ingest_network_entry(NetworkEntry {
        url: "https://app.example.com/api/users".into(),
        method: "GET".into(),
        status: 200,
        start_ms: 0.0,
        ttfb_ms: 40.0,
        download_ms: 10.0,
        bytes: 512,
        initiator: Some("https://app.example.com/".into()),
        render_blocking: true,
        ..Default::default()
    });
    bridge.ingest_network_entry(NetworkEntry {
        url: "https://cdn.thirdparty.io/tag.js".into(),
        method: "GET".into(),
        status: 200,
        bytes: 8192,
        ..Default::default()
    });

    assert_eq!(bridge.len(), 2);
    // Perf waterfall / initiator graph derivations.
    assert_eq!(bridge.resource_timings().len(), 2);
    assert_eq!(bridge.initiated_resources().len(), 2);
    // Replayable API requests for the backend/security engines.
    let reqs = bridge.api_requests();
    assert_eq!(reqs[0].method, "GET");
    assert_eq!(reqs[0].url, "https://app.example.com/api/users");
    // Third-party attribution weight.
    let tp = bridge.third_party_bytes("app.example.com");
    assert_eq!(tp.get("cdn.thirdparty.io"), Some(&8192));
    assert!(!tp.contains_key("app.example.com"));

    bridge.clear();
    assert!(bridge.is_empty());
}

#[test]
fn test_cdp_bridge_ingests_tamper_history() {
    let bridge = CdpBridge::new();
    let mut req_headers = HashMap::new();
    req_headers.insert("authorization".to_string(), "Bearer x".to_string());
    let mut resp_headers = HashMap::new();
    resp_headers.insert("X-Cache".to_string(), "HIT".to_string());

    let req = InterceptedRequest {
        id: "1".into(),
        timestamp: 100,
        method: "POST".into(),
        url: "https://api.example.com/login".into(),
        headers: req_headers,
        body: Some("{\"u\":\"a\"}".into()),
        is_modified: false,
    };
    let resp = InterceptedResponse {
        request_id: "1".into(),
        timestamp: 150,
        status_code: 200,
        headers: resp_headers,
        body: Some("ok".into()),
        duration_ms: 50,
    };

    bridge.ingest_tamper_history(&[(req, Some(resp))]);
    assert_eq!(bridge.len(), 1);
    let entries = bridge.entries();
    assert_eq!(entries[0].method, "POST");
    assert_eq!(entries[0].status, 200);
    assert!(entries[0].from_cache, "X-Cache: HIT should mark from_cache");
    // Tamper traffic becomes replayable API requests carrying the captured headers.
    let reqs = bridge.api_requests();
    assert_eq!(reqs[0].method, "POST");
    assert_eq!(reqs[0].url, "https://api.example.com/login");
    assert!(reqs[0].headers.contains_key("authorization"));
}

// ===========================================================================
// 3. Backend active halves — webhook sink, chain runner, IDOR sender
// ===========================================================================

#[test]
fn test_backend_webhook_sink_receives_real_post() {
    // The sink loop records into the process-wide engine, so drive that one.
    let backend = AppState::global().features().backend();
    let port = backend
        .start_webhook_sink(0)
        .expect("start webhook sink on ephemeral port");
    assert_eq!(backend.webhook_sink_port(), Some(port));

    let unique_path = format!("/thor-hook-{port}");
    let resp = raw_http(port, "POST", &unique_path, "{\"event\":\"ping\"}")
        .expect("POST to webhook sink");
    assert!(resp.contains("200 OK"), "sink should ack with 200: {resp}");

    // The inbound POST is recorded asynchronously by the listener thread.
    let event = poll_until(Duration::from_secs(3), || {
        backend.webhooks_matching(&unique_path).into_iter().next()
    });
    backend.stop_webhook_sink();

    let event = event.expect("webhook event must be recorded");
    assert_eq!(event.method, "POST");
    assert_eq!(event.path, unique_path);
    assert!(event.body.contains("ping"), "body captured: {}", event.body);
}

#[test]
fn test_backend_chain_runner_executes_and_threads_captures() {
    // Server returns a token that step 1 captures and step 2 consumes.
    let server = TestServer::start(200, r#"{"token":"secret-123"}"#);
    let base = server.url.clone();

    let backend = backend::BackendEngine::new();
    let mut login = ChainStep {
        name: "login".into(),
        request: ApiRequest {
            method: "POST".into(),
            url: format!("{base}/login"),
            ..Default::default()
        },
        captures: HashMap::new(),
    };
    login.captures.insert("token".into(), "token".into());
    let me = ChainStep {
        name: "me".into(),
        request: ApiRequest {
            method: "GET".into(),
            url: format!("{base}/me?t={{{{token}}}}"),
            ..Default::default()
        },
        captures: HashMap::new(),
    };

    let result = backend.run_chain(&[login, me]).expect("chain executes");
    assert!(result.ok, "both steps returned 2xx");
    assert_eq!(result.steps.len(), 2);
    assert_eq!(result.steps[0].status, 200);
    assert_eq!(result.steps[1].status, 200);
    assert!(result.steps[0].captured.contains(&"token".to_string()));
    assert_eq!(result.vars.get("token").map(String::as_str), Some("secret-123"));
}

#[test]
fn test_backend_idor_matrix_sends_across_contexts() {
    // Any context receiving 200 on a resource it does not own is a BOLA finding.
    let server = TestServer::start(200, r#"{"ok":true}"#);
    let backend = backend::BackendEngine::new();
    backend.store_auth(
        "owner",
        AuthScheme::Bearer {
            token: "owner-tok".into(),
        },
    );
    backend.store_auth(
        "attacker",
        AuthScheme::Bearer {
            token: "attacker-tok".into(),
        },
    );

    let req = ApiRequest {
        method: "GET".into(),
        url: format!("{}/users/1/profile", server.url),
        ..Default::default()
    };
    let report = backend.run_idor_matrix(
        &req,
        &["owner".to_string(), "attacker".to_string()],
        Some("owner"),
    );

    // Both contexts were actually sent the request.
    assert_eq!(report.probes.len(), 2);
    assert!(report.probes.iter().all(|p| p.status == 200));
    // Only the non-owner receiving 200 is flagged.
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].identity, "attacker");
}

// ===========================================================================
// 4. Security — CORS/CSP findings, consent-gated fuzz, OOB listener
// ===========================================================================

#[test]
fn test_security_header_analyzer_flags_cors_and_csp_weaknesses() {
    let se = security::SecurityEngine::new();
    let mut headers = BTreeMap::new();
    headers.insert(
        "content-security-policy".to_string(),
        "default-src 'self' 'unsafe-inline' 'unsafe-eval'".to_string(),
    );
    headers.insert(
        "access-control-allow-origin".to_string(),
        "*".to_string(),
    );
    headers.insert(
        "access-control-allow-credentials".to_string(),
        "true".to_string(),
    );

    let report = se.analyze_headers(&headers);
    // CSP content weaknesses are surfaced.
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.recommendation.to_lowercase().contains("unsafe-inline")),
        "should flag CSP unsafe-inline"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.recommendation.to_lowercase().contains("unsafe-eval")),
        "should flag CSP unsafe-eval"
    );
    // Wildcard ACAO with credentials is a critical CORS misconfig.
    assert!(
        report.findings.iter().any(|f| {
            f.header == "access-control-allow-origin"
                && f.recommendation.to_lowercase().contains("cors")
        }),
        "should flag wildcard CORS + credentials: {:?}",
        report.findings
    );
}

#[test]
fn test_security_fuzz_is_consent_gated_by_scope() {
    let se = security::SecurityEngine::new();

    // A public target is out of scope -> refused.
    let public = se.build_fuzz_cases_gated(
        "https://example.com/search?q=FUZZ",
        FuzzCategory::Xss,
        "https://example.com/search?q=FUZZ",
    );
    assert!(public.is_err(), "public target must be rejected");
    assert!(public.unwrap_err().to_lowercase().contains("blocked"));

    // A cloud-metadata abuse target is refused even though 169.254.x is link-local.
    let metadata = se.build_fuzz_cases_gated(
        "http://169.254.169.254/latest/meta-data/FUZZ",
        FuzzCategory::Ssti,
        "http://169.254.169.254/latest/meta-data/FUZZ",
    );
    assert!(metadata.is_err(), "metadata endpoint must be rejected");

    // A private/local target is in scope -> cases produced.
    let local = se
        .build_fuzz_cases_gated(
            "http://127.0.0.1:8080/api?x=FUZZ",
            FuzzCategory::Xss,
            "http://127.0.0.1:8080/api?x=FUZZ",
        )
        .expect("local target allowed");
    assert!(!local.is_empty());
    assert!(local.iter().all(|c| c.mutated.contains("127.0.0.1")));
    assert!(local.iter().any(|c| !c.mutated.contains("FUZZ")));
}

#[test]
fn test_security_oob_listener_records_http_callback() {
    // The OOB loop records into the process-wide engine.
    let security = AppState::global().features().security();
    let port = security
        .start_oob_listener(0)
        .expect("start OOB listener on ephemeral port");
    assert_eq!(security.oob_listener_port(), Some(port));

    let token = "thoroobtoken";
    let resp = raw_http(port, "GET", &format!("/{token}"), "").expect("hit OOB listener");
    assert!(resp.contains("200 OK"));

    let hit = poll_until(Duration::from_secs(3), || {
        security.oob_interactions(token).into_iter().next()
    });
    security.stop_oob_listener();

    let hit = hit.expect("OOB interaction must be recorded");
    assert_eq!(hit.token, token);
    assert_eq!(hit.protocol, "http");
    assert!(hit.detail.contains(token) || hit.detail.contains("callback"));
}

// ===========================================================================
// 5. QA — flake replay rate, masked visual regression
// ===========================================================================

#[test]
fn test_qa_flake_radar_replay_returns_rate() {
    let server = TestServer::start(200, r#"{"ok":true}"#);
    let scenario = ThorScenario::new("flake-check", &server.url)
        .with_runner(RunnerType::Protocol)
        .with_vus(1)
        .with_duration(1)
        .add_step(ScenarioStep::Navigate {
            url: server.url.clone(),
        });

    // Sanity: a single protocol run against the live server succeeds.
    let single = ProtocolRunner::new().run_blocking(&scenario);
    assert!(single.is_ok(), "protocol runner should reach local server");

    let qe = qa::QaEngine::new();
    let report = qe.flake_radar_replay(&scenario, 3);
    assert_eq!(report.requested_runs, 3);
    assert_eq!(report.results.len(), 3);
    assert_eq!(report.radar.runs, 3);
    // The pass rate is a real fraction derived from the replayed runs.
    assert!(report.radar.pass_rate >= 0.0 && report.radar.pass_rate <= 1.0);
    assert!(!report.radar.classification.is_empty());
}

#[test]
fn test_qa_visual_regression_mask_skips_regions() {
    // 8x8 grayscale buffers. Introduce a large difference confined to a corner.
    let width = 8usize;
    let height = 8usize;
    let baseline = vec![100u8; width * height];
    let mut candidate = baseline.clone();
    // Corrupt the top-left 3x3 block so it dominates an unmasked diff.
    for row in 0..3 {
        for col in 0..3 {
            candidate[row * width + col] = 255;
        }
    }

    let qe = qa::QaEngine::new();
    // Without masking the change fails a tight threshold.
    let unmasked = qe.visual_regression(&baseline, &candidate, 0.01);
    assert!(!unmasked.passed, "unmasked diff should fail");
    assert_eq!(unmasked.masked_pixels, 0);

    // Masking the volatile region excludes it from the comparison -> pass.
    let mask = MaskRegion {
        selector: ".timestamp".into(),
        x: 0,
        y: 0,
        width: 3,
        height: 3,
    };
    let masked = qe.visual_regression_masked(&baseline, &candidate, width, 0.01, &[mask]);
    assert!(masked.passed, "masked diff should pass: {:?}", masked);
    assert_eq!(masked.masked_pixels, 9, "3x3 region excluded");
    assert!(masked.pixel_diff_ratio < unmasked.pixel_diff_ratio);
}

// ===========================================================================
// 6. AI — PII detection, prompt version timeline, eval from session
// ===========================================================================

#[test]
fn test_ai_injection_scanner_detects_pii() {
    let ae = ai::AiEngine::new();
    let findings =
        ae.scan_injection("contact me at alice@example.com or use key sk-ABCDEF0123456789ABCDEF");
    assert!(!findings.is_empty(), "PII should be detected");
    // At least one finding is classified as PII (email / api-key / jwt).
    assert!(
        findings.iter().any(|f| f.category == "pii"),
        "expected a pii-category finding: {:?}",
        findings
    );
    assert!(
        findings
            .iter()
            .any(|f| f.pattern.starts_with("pii:")),
        "pii findings carry a pii:* pattern"
    );
}

#[test]
fn test_ai_prompt_timeline_tracks_versions_with_diffs() {
    let ae = ai::AiEngine::new();
    let name = "system-prompt";
    let v1 = ae.record_prompt_version(name, "You are a helpful assistant.");
    assert_eq!(v1.version, 1);
    assert!(v1.diff_from_prev.is_none(), "first version has no prior diff");

    let v2 = ae.record_prompt_version(name, "You are a concise, helpful assistant.");
    assert_eq!(v2.version, 2);
    let diff = v2.diff_from_prev.expect("v2 diffs against v1");
    assert!(
        !diff.added.is_empty() || !diff.removed.is_empty(),
        "the revision should produce a non-empty diff"
    );

    let timeline = ae.prompt_timeline(name);
    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].version, 1);
    assert_eq!(timeline[1].version, 2);
    // Unknown prompt -> empty timeline.
    assert!(ae.prompt_timeline("never-recorded").is_empty());
}

#[test]
fn test_ai_eval_from_session_scores_recorded_calls() {
    let mut session = RecordedSession::new("Checkout eval", "https://shop.example.com");
    session.add_trace(NetworkTrace {
        method: "GET".into(),
        url: "https://shop.example.com/cart".into(),
        headers: HashMap::new(),
        body: None,
        status: 200,
    });
    session.add_trace(NetworkTrace {
        method: "POST".into(),
        url: "https://shop.example.com/checkout".into(),
        headers: HashMap::new(),
        body: Some("{}".into()),
        status: 500,
    });

    let ae = ai::AiEngine::new();
    let result = ae.eval_from_session(&session);
    // One success (200) and one failure (500) become two eval cases.
    assert_eq!(result.total, 2);
    assert_eq!(result.passed, 1);
    assert!((result.pass_rate - 0.5).abs() < 1e-6);
    assert_eq!(result.failures.len(), 1);
}
