//! Wave-2 feature coverage for the Thor Engineer Feature Suite.
//!
//! Persona-grouped smoke tests for QA wave-2 analyzers, plus orchestrator
//! invoke checks for the new slugs. Registry count is asserted as
//! `FeatureId::ALL.len()` so coder/perf expansions to ~144 compose cleanly.

use std::collections::HashMap;

use serde_json::json;

use thor::dx::test_generator::NetworkTrace;
use thor::features::orchestrator::FeatureOrchestrator;
use thor::features::qa::{
    self, A11yViolation, CoverageEdge, CoveredStep, ElementSignals, FlakeVector, HarMockEntry,
    HealSignal, OracleObservation, QuarantineInput, ShardScenario, StorageSeed,
};
use thor::features::{FeatureCategory, FeatureEngine, FeatureId};
use thor::load::BrowserMetrics;

fn registry_len() -> usize {
    FeatureId::ALL.len()
}

#[test]
fn test_wave2_registry_includes_qa24() {
    let engine = FeatureEngine::new();
    let n = registry_len();
    assert!(n >= 84, "wave-2 QA expands registry to at least 84, got {n}");
    assert_eq!(engine.registry().len(), n);
    assert_eq!(engine.registry().by_category(FeatureCategory::Qa).len(), 24);

    for slug in [
        "flake-triager",
        "quarantine-policy",
        "contract-drift-gate",
        "hermetic-fixture-pack",
        "a11y-test-gen",
        "snapshot-strategy-pack",
        "coverage-adjacency-map",
        "parallel-shard-planner",
        "trace-property-miner",
        "runtime-self-heal",
        "oracle-synthesizer",
        "mutation-hook-bridge",
    ] {
        let desc = engine
            .registry()
            .by_slug(slug)
            .unwrap_or_else(|| panic!("missing slug {slug}"));
        assert_eq!(desc.category, FeatureCategory::Qa);
        assert!(engine.is_enabled(desc.id));
        assert_eq!(engine.record_invocation(desc.id), Some(1));
    }
}

#[test]
fn test_qa_wave2_engine_methods() {
    let qe = qa::QaEngine::new();

    let radar = qe.flake_radar("checkout", &[true, false, true, false, false]);
    qe.record_trace_frame("checkout", "click pay", "click", "<btn/>");
    qe.record_trace_frame("checkout", "confirm", "click", "<modal/>");
    let triage = qe.triage_flake(
        "checkout",
        &radar,
        &FlakeVector {
            timing_cv: 0.7,
            race_score: 0.4,
            ..Default::default()
        },
        &qe.trace_frames("checkout"),
    );
    assert_eq!(triage.root_cause, "timing-race");
    assert!(!triage.evidence_frame_indexes.is_empty());

    let q = qe.evaluate_quarantine(&QuarantineInput {
        test: "checkout".into(),
        flake_rate: 0.55,
        consecutive_fails: 4,
        days_in_quarantine: 0.0,
        green_streak: 0,
        currently_quarantined: false,
    });
    assert_eq!(q.action, "quarantine");
    assert!(q.quarantine_yaml.contains("checkout"));

    let baseline = vec![qa::ContractInteraction {
        description: "cart".into(),
        method: "GET".into(),
        path: "/cart".into(),
        expected_status: 200,
        response_schema: Some(json!({"type": "object", "required": ["items"]})),
    }];
    let live = vec![
        qa::ActualInteraction {
            method: "GET".into(),
            path: "/cart".into(),
            status: 200,
            body: Some(json!({"items": []})),
        },
        qa::ActualInteraction {
            method: "POST".into(),
            path: "/cart/add".into(),
            status: 201,
            body: None,
        },
    ];
    let drift = qe.contract_drift_gate(&baseline, &live);
    assert!(!drift.breaking);
    assert!(drift.changes.iter().any(|c| c.kind == "added"));

    let pack = qe.build_hermetic_fixture_pack(
        "buyer",
        qa::IdentityFixture {
            name: "buyer".into(),
            role: "user".into(),
            ..Default::default()
        },
        vec![HarMockEntry {
            method: "GET".into(),
            url: "/cart".into(),
            status: 200,
            body: "{\"items\":[]}".into(),
        }],
        vec![StorageSeed {
            origin: "https://shop".into(),
            local_storage: HashMap::from([("cartId".into(), "1".into())]),
            ..Default::default()
        }],
    );
    assert!(!pack.pack_digest.is_empty());

    let a11y = qe.generate_a11y_spec(
        "cart",
        &[A11yViolation {
            rule_id: "button-name".into(),
            selector: "button.pay".into(),
            impact: "critical".into(),
            message: "Buttons must have discernible text".into(),
        }],
    );
    assert!(a11y.playwright_ts.contains("@axe-core/playwright"));
    assert_eq!(a11y.violation_count, 1);

    let snap = qe.snapshot_strategy_pack("cart", &[9u8; 32], "<div id=cart/>", "tree", "{}");
    assert_eq!(snap.layers.len(), 4);

    let observed = vec![
        CoverageEdge {
            kind: "route".into(),
            from: "/".into(),
            to: "/cart".into(),
        },
        CoverageEdge {
            kind: "component".into(),
            from: "Cart".into(),
            to: "PayButton".into(),
        },
        CoverageEdge {
            kind: "api".into(),
            from: "PayButton".into(),
            to: "POST /pay".into(),
        },
    ];
    let covered = vec![observed[0].clone(), observed[1].clone()];
    let cov = qe.coverage_adjacency(&observed, &covered);
    assert_eq!(cov.uncovered.len(), 1);
    assert!((cov.coverage_ratio - 2.0 / 3.0).abs() < 1e-9);

    let plan = qe.plan_parallel_shards(
        &[
            ShardScenario {
                name: "admin-a".into(),
                duration_ms: 300.0,
                identity: "admin".into(),
            },
            ShardScenario {
                name: "user-a".into(),
                duration_ms: 100.0,
                identity: "user".into(),
            },
            ShardScenario {
                name: "admin-b".into(),
                duration_ms: 50.0,
                identity: "admin".into(),
            },
        ],
        2,
    );
    assert_eq!(plan.shard_count, 2);
    assert!(plan.plan_json.contains("\"shard_count\": 2"));

    let mut headers = HashMap::new();
    headers.insert("Authorization".into(), "Bearer t".into());
    let mined = qe.mine_trace_properties(&[NetworkTrace {
        method: "POST".into(),
        url: "https://shop/admin/orders?page=2".into(),
        headers,
        body: None,
        status: 200,
    }]);
    assert!(mined.iter().any(|c| c.kind == "idempotency"));
    assert!(mined.iter().any(|c| c.kind == "pagination"));
    assert!(mined.iter().any(|c| c.kind == "authz"));

    let heal = qe.runtime_self_heal(
        &ElementSignals {
            test_id: Some("pay".into()),
            role: Some("button".into()),
            accessible_name: Some("Pay".into()),
            css: Some(".pay".into()),
            ..Default::default()
        },
        &[HealSignal {
            locator: "page.getByTestId('pay')".into(),
            strategy: "test-id".into(),
            successes: 1,
            failures: 8,
            last_error: "detached".into(),
        }],
    );
    assert!(!heal.patch_json.is_empty());

    let mut obs = OracleObservation {
        dom_digest: Some("deadbeef".into()),
        status: Some(201),
        vitals: Some(BrowserMetrics {
            lcp_ms: 1800.0,
            cls: 0.02,
            ..Default::default()
        }),
        ..Default::default()
    };
    obs.headers
        .insert("x-request-id".into(), "abc".into());
    obs.storage.insert("orderId".into(), "42".into());
    let oracles = qe.synthesize_oracles(&obs);
    assert!(oracles.len() >= 5);

    let hooks = qe.mutation_hook_bridge(&[CoveredStep {
        id: "pay-click".into(),
        file: "src/checkout.rs".into(),
        mutation_targets: vec!["checkout::charge".into()],
    }]);
    assert_eq!(hooks.covered_steps, 1);
    assert_eq!(hooks.stryker["mutate"][0], "src/checkout.rs");
}

#[test]
fn test_orchestrator_invokes_qa_wave2_slugs() {
    let triager = FeatureOrchestrator::invoke(
        "flake-triager",
        &json!({
            "test": "x",
            "results": [true, false, true, false],
            "vectors": { "locator_churn": 0.95 }
        }),
    )
    .expect("flake-triager");
    assert_eq!(triager["slug"], "flake-triager");
    assert_eq!(triager["result"]["root_cause"], "locator-instability");

    let q = FeatureOrchestrator::invoke(
        "quarantine-policy",
        &json!({
            "input": {
                "test": "x",
                "flake_rate": 0.1,
                "consecutive_fails": 0,
                "days_in_quarantine": 20.0,
                "green_streak": 0,
                "currently_quarantined": true
            }
        }),
    )
    .expect("quarantine");
    let q_action = q["result"]["action"]
        .as_str()
        .or_else(|| q["result"]["decision"]["action"].as_str());
    assert_eq!(q_action, Some("expire"), "{q}");

    let drift = FeatureOrchestrator::invoke(
        "contract-drift-gate",
        &json!({
            "baseline": [{
                "description": "g",
                "method": "GET",
                "path": "/a",
                "expected_status": 200
            }],
            "live": [{
                "method": "GET",
                "path": "/a",
                "status": 404
            }]
        }),
    )
    .expect("drift");
    assert_eq!(drift["result"]["breaking"], true);

    let pack = FeatureOrchestrator::invoke(
        "hermetic-fixture-pack",
        &json!({
            "name": "p",
            "identity": { "name": "u", "role": "user" },
            "har_mocks": [],
            "storage": []
        }),
    )
    .expect("pack");
    assert!(pack["result"]["pack_digest"].as_str().unwrap().len() > 0);

    let a11y = FeatureOrchestrator::invoke(
        "a11y-test-gen",
        &json!({
            "page_name": "home",
            "violations": [{
                "rule_id": "label",
                "selector": "input",
                "impact": "serious",
                "message": "missing"
            }]
        }),
    )
    .expect("a11y");
    assert!(a11y["result"]["playwright_ts"]
        .as_str()
        .unwrap()
        .contains("AxeBuilder"));

    let snap = FeatureOrchestrator::invoke(
        "snapshot-strategy-pack",
        &json!({
            "name": "home",
            "pixels": [1, 2, 3, 4],
            "dom": "<div/>",
            "a11y_digest": "x",
            "body": "{}"
        }),
    )
    .expect("snap");
    assert_eq!(snap["result"]["layers"].as_array().unwrap().len(), 4);

    let cov = FeatureOrchestrator::invoke(
        "coverage-adjacency-map",
        &json!({
            "observed": [{"kind": "api", "from": "A", "to": "B"}],
            "covered": []
        }),
    )
    .expect("cov");
    assert_eq!(cov["result"]["uncovered"].as_array().unwrap().len(), 1);

    let shards = FeatureOrchestrator::invoke(
        "parallel-shard-planner",
        &json!({
            "shard_count": 2,
            "scenarios": [
                {"name": "a", "duration_ms": 10.0, "identity": "admin"},
                {"name": "b", "duration_ms": 20.0, "identity": "user"}
            ]
        }),
    )
    .expect("shards");
    assert_eq!(shards["result"]["shard_count"], 2);

    let mined = FeatureOrchestrator::invoke(
        "trace-property-miner",
        &json!({
            "traces": [{
                "method": "PUT",
                "url": "https://x/items/1",
                "headers": {},
                "status": 200
            }]
        }),
    )
    .expect("miner");
    assert!(mined["result"].as_array().unwrap().iter().any(|c| c["kind"] == "idempotency"));

    let heal = FeatureOrchestrator::invoke(
        "runtime-self-heal",
        &json!({
            "signals": { "test_id": "ok", "css": ".ok" },
            "live": [{
                "locator": "page.getByTestId('ok')",
                "strategy": "test-id",
                "successes": 5,
                "failures": 0,
                "last_error": ""
            }]
        }),
    )
    .expect("heal");
    assert!(heal["result"]["patch_json"].as_str().unwrap().contains("preferred"));

    let oracles = FeatureOrchestrator::invoke(
        "oracle-synthesizer",
        &json!({
            "observation": {
                "status": 200,
                "dom_digest": "abc",
                "headers": { "content-type": "application/json" }
            }
        }),
    )
    .expect("oracles");
    assert!(oracles["result"].as_array().unwrap().len() >= 3);

    let muts = FeatureOrchestrator::invoke(
        "mutation-hook-bridge",
        &json!({
            "steps": [{
                "id": "s1",
                "file": "src/a.rs",
                "mutation_targets": ["a::f"]
            }]
        }),
    )
    .expect("muts");
    assert_eq!(muts["result"]["covered_steps"], 1);
}

// ---------------------------------------------------------------------------
// P0-13 — QA composition (FlakeVector from traces, hermetic export, quarantine)
// ---------------------------------------------------------------------------

#[test]
fn test_qa_flake_vector_from_traces_and_triager() {
    let frames = vec![
        qa::TraceFrame {
            index: 0,
            step: "a".into(),
            action: "click".into(),
            dom_digest: "d0".into(),
            dom_snapshot: "<a/>".into(),
            timestamp_ms: 1_000,
        },
        qa::TraceFrame {
            index: 1,
            step: "b".into(),
            action: "click".into(),
            dom_digest: "d1".into(),
            dom_snapshot: "<b/>".into(),
            timestamp_ms: 1_008,
        },
        qa::TraceFrame {
            index: 2,
            step: "c".into(),
            action: "click".into(),
            dom_digest: "d2".into(),
            dom_snapshot: "<c/>".into(),
            timestamp_ms: 1_016,
        },
    ];
    let derived = FlakeVector::from_trace_frames(&frames);
    assert!(!derived.is_empty());
    assert!(derived.locator_churn > 0.5);

    let mut headers = HashMap::new();
    headers.insert("x-response-time".into(), "12ms".into());
    let mut headers2 = HashMap::new();
    headers2.insert("x-response-time".into(), "90ms".into());
    let net = FlakeVector::from_network_traces(&[
        NetworkTrace {
            method: "GET".into(),
            url: "https://shop/pay".into(),
            headers,
            body: None,
            status: 200,
        },
        NetworkTrace {
            method: "GET".into(),
            url: "https://shop/pay".into(),
            headers: headers2,
            body: None,
            status: 200,
        },
    ]);
    assert!(net.network_jitter_ms > 0.0);

    // Orchestrator path: empty vectors → derive from frames/traces.
    let out = FeatureOrchestrator::invoke(
        "flake-triager",
        &json!({
            "test": "pay",
            "results": [true, false, true, false],
            "frames": frames,
            "traces": [{
                "method": "GET",
                "url": "https://shop/pay",
                "headers": { "x-response-time": "12ms" },
                "status": 200
            }, {
                "method": "GET",
                "url": "https://shop/pay",
                "headers": { "x-response-time": "90ms" },
                "status": 500
            }]
        }),
    )
    .expect("derived flake-triager");
    assert!(!out["result"]["root_cause"].as_str().unwrap().is_empty());
    assert_ne!(out["result"]["root_cause"], "unknown");
}

#[test]
fn test_qa_hermetic_fixture_playwright_roundtrip() {
    let qe = qa::QaEngine::new();
    let mut identity = qa::IdentityFixture {
        name: "buyer".into(),
        role: "user".into(),
        ..Default::default()
    };
    identity.cookies.insert("sid".into(), "s1".into());
    let pack = qe.build_hermetic_fixture_pack(
        "buyer-pack",
        identity,
        vec![HarMockEntry {
            method: "GET".into(),
            url: "/api/cart".into(),
            status: 200,
            body: "{\"items\":[]}".into(),
        }],
        vec![StorageSeed {
            origin: "https://shop".into(),
            local_storage: HashMap::from([("cartId".into(), "9".into())]),
            ..Default::default()
        }],
    );
    let pw = pack.to_playwright();
    assert!(pw.contains("route.fulfill"));
    assert!(pw.contains("storageState"));
    let state: serde_json::Value =
        serde_json::from_str(&pack.storage_state_json()).expect("round-trip JSON");
    assert_eq!(state["cookies"][0]["name"], "sid");
    assert_eq!(state["origins"][0]["origin"], "https://shop");

    let orch = FeatureOrchestrator::invoke(
        "hermetic-fixture-pack",
        &json!({
            "name": "buyer-pack",
            "identity": { "name": "buyer", "role": "user", "cookies": { "sid": "s1" } },
            "har_mocks": [{ "method": "GET", "url": "/api/cart", "status": 200, "body": "{}" }],
            "storage": [{ "origin": "https://shop", "local_storage": { "cartId": "9" } }],
            "export": "playwright"
        }),
    )
    .expect("hermetic export");
    assert!(orch["result"]["playwright_ts"]
        .as_str()
        .unwrap()
        .contains("route.fulfill"));
    assert!(orch["result"]["storage_state_json"]
        .as_str()
        .unwrap()
        .contains("cartId"));
}

#[test]
fn test_qa_quarantine_persist_and_ci_filter() {
    let tmp = std::env::temp_dir().join(format!(
        "thor-wave2-q-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::env::set_var("THOR_DATA_DIR", &tmp);

    let qe = qa::QaEngine::new();
    let d = qe.evaluate_quarantine(&QuarantineInput {
        test: "checkout.spec.ts".into(),
        flake_rate: 0.6,
        consecutive_fails: 4,
        days_in_quarantine: 0.0,
        green_streak: 0,
        currently_quarantined: false,
    });
    assert_eq!(d.action, "quarantine");
    let path = qe.persist_quarantine().expect("persist quarantine");
    assert_eq!(path, tmp.join("quarantine.yaml"));
    assert!(path.exists());
    let yaml = qa::QaEngine::load_quarantine_yaml().unwrap();
    assert!(yaml.contains("checkout.spec.ts"));
    let filter = qe.ci_quarantine_filter();
    assert!(filter.contains("--grep-invert"));
    assert!(filter.contains("checkout"));

    // TestGenerator helpers agree with engine YAML.
    let titles = thor::dx::test_generator::TestGenerator::quarantined_titles_from_yaml(&yaml);
    assert!(titles.iter().any(|t| t.contains("checkout")));

    std::env::remove_var("THOR_DATA_DIR");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_qa_a11y_and_oracle_composition() {
    let qe = qa::QaEngine::new();
    let a11y = qe.generate_a11y_spec(
        "home",
        &[A11yViolation {
            rule_id: "label".into(),
            selector: "input#email".into(),
            impact: "serious".into(),
            message: "Form elements must have labels".into(),
        }],
    );
    // Shares scaffold DNA with TestGenerator::a11y_spec_scaffold.
    let scaffold = thor::dx::test_generator::TestGenerator::a11y_spec_scaffold(
        "home",
        &["label".into()],
    );
    assert!(a11y.playwright_ts.contains("AxeBuilder"));
    assert!(scaffold.contains("AxeBuilder"));
    assert!(a11y.playwright_ts.contains("input#email"));

    let session = thor::dx::test_generator::RecordedSession::new("Home", "https://x");
    let composed = qe.export_with_oracles(
        &session,
        &OracleObservation {
            status: Some(200),
            dom_digest: Some("deadbeef".into()),
            ..Default::default()
        },
    );
    assert!(composed.contains("Thor synthesized oracles"));
    assert!(composed.contains("toBe(200)"));

    let oracles = qe.synthesize_oracles(&OracleObservation {
        status: Some(201),
        ..Default::default()
    });
    let lines: Vec<String> = oracles.iter().map(|o| o.assertion.clone()).collect();
    let appended = thor::dx::test_generator::TestGenerator::append_oracles(
        "test('t', async ({ page }) => {\n  await page.goto('/');\n});\n",
        &lines,
    );
    assert!(appended.contains("toBe(201)"));
}

// ---------------------------------------------------------------------------
// P0-15 — Cross-domain wave2 orchestrator invokes (real-shaped payloads)
// ---------------------------------------------------------------------------

#[test]
fn test_wave2_security_backend_invokes() {
    assert_eq!(
        FeatureEngine::new()
            .registry()
            .by_category(FeatureCategory::Security)
            .len(),
        24
    );

    // webhook-signature-vault — matching HMAC-SHA256 (Coder P0 deepen).
    let sig = FeatureOrchestrator::invoke(
        "webhook-signature-vault",
        &json!({
            "input": {
                "payload": "hello",
                "secret": "sekrit",
                "provided_signature": "sha256=3ffea2c7e630ed8f52654e8e7328870035fdf02ac33d381a2fe2d20510d2df96",
                "algorithm": "sha256"
            }
        }),
    )
    .expect("webhook-signature-vault");
    assert_eq!(sig["result"]["valid"], true, "{sig}");

    let cors = FeatureOrchestrator::invoke(
        "cors-deep-probe",
        &json!({
            "probes": [{
                "origin": "https://evil.example",
                "acao": "*",
                "acac": "true",
                "method": "GET"
            }]
        }),
    )
    .expect("cors-deep-probe");
    assert_eq!(cors["result"]["credentials_with_star"], true);
    assert!(!cors["result"]["findings"].as_array().unwrap().is_empty());

    let fix = FeatureOrchestrator::invoke(
        "session-fixation-lab",
        &json!({
            "probe": {
                "pre_auth_session": "SAMESID",
                "post_auth_session": "SAMESID",
                "login_status": 200,
                "target_url": "http://127.0.0.1/login"
            }
        }),
    )
    .expect("session-fixation-lab");
    assert_eq!(fix["result"]["vulnerable"], true);

    // ssrf-lab-harness — consent Err on public target without lab_consent.
    let ssrf_err = FeatureOrchestrator::invoke(
        "ssrf-lab-harness",
        &json!({
            "lab_consent": false,
            "targets": [
                { "url": "https://example.com/open", "resolved_ip": null }
            ]
        }),
    );
    let ssrf_msg = ssrf_err.expect_err("ssrf public without consent");
    assert!(
        ssrf_msg.contains("lab_consent") || ssrf_msg.contains("refused"),
        "unexpected ssrf err: {ssrf_msg}"
    );

    // Abuse metadata still analyzable with consent (blocked list, not Err).
    let ssrf = FeatureOrchestrator::invoke(
        "ssrf-lab-harness",
        &json!({
            "lab_consent": true,
            "targets": [
                { "url": "http://169.254.169.254/latest/meta-data/", "resolved_ip": "169.254.169.254" },
                { "url": "http://127.0.0.1:8080/admin", "resolved_ip": "127.0.0.1" }
            ]
        }),
    )
    .expect("ssrf-lab-harness");
    assert!(
        !ssrf["result"]["blocked"].as_array().unwrap().is_empty(),
        "metadata target should be blocked"
    );
    assert!(ssrf["result"]["allowed_lab"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u.as_str().unwrap().contains("127.0.0.1")));

    // Also exercise fuzz-repeater consent Err.
    let consent_err = FeatureOrchestrator::invoke(
        "fuzz-repeater",
        &json!({
            "template": "/q?x=FUZZ",
            "category": "Xss",
            "target_url": "https://example.com/search"
        }),
    );
    let err = consent_err.expect_err("public fuzz must Err without consent");
    assert!(
        err.contains("blocked") || err.contains("private") || err.contains("local"),
        "unexpected consent err: {err}"
    );

    let stub = FeatureOrchestrator::invoke(
        "live-stub-server",
        &json!({
            "routes": [
                { "method": "GET", "path": "/api/me", "status": 200, "body": "{}" },
                { "method": "GET", "path": "/api/me", "status": 200, "body": "{}" },
                { "method": "GET", "path": "/users/{id}", "status": 200, "body": "{}" }
            ]
        }),
    )
    .expect("live-stub-server");
    assert_eq!(stub["result"]["routes"], 3);
    assert!(!stub["result"]["collisions"].as_array().unwrap().is_empty());
    assert!(!stub["result"]["open_paths"].as_array().unwrap().is_empty());

    let rl = FeatureOrchestrator::invoke(
        "rate-limit-probe",
        &json!({
            "samples": [
                { "status": 200, "retry_after_secs": null, "remaining": 2, "limit": 10, "timestamp_ms": 1 },
                { "status": 429, "retry_after_secs": 1.5, "remaining": 0, "limit": 10, "timestamp_ms": 2 }
            ]
        }),
    )
    .expect("rate-limit-probe");
    assert_eq!(rl["result"]["limited_count"], 1);
    assert_eq!(rl["result"]["inferred_limit"], 10);

    let oapi = FeatureOrchestrator::invoke(
        "openapi-diff-gate",
        &json!({
            "before": [{ "path": "/v1/users", "method": "GET", "operation_id": "listUsers" }],
            "after": [{ "path": "/v1/users", "method": "GET", "operation_id": "listUsersV2" }]
        }),
    )
    .expect("openapi-diff-gate");
    assert!(!oapi["result"]["changed"].as_array().unwrap().is_empty());
}

#[test]
fn test_wave2_frontend_invokes() {
    let dom = FeatureOrchestrator::invoke(
        "dom-snapshot-capture",
        &json!({
            "root": {
                "tag": "div",
                "attributes": { "id": "app" },
                "text": null,
                "children": [{
                    "tag": "img",
                    "attributes": { "src": "/hero.png", "alt": "" },
                    "children": []
                }]
            }
        }),
    )
    .expect("dom-snapshot-capture");
    assert!(dom["result"]["node_count"].as_u64().unwrap() >= 2);
    assert!(!dom["result"]["digest_hex"].as_str().unwrap().is_empty());

    let hyd = FeatureOrchestrator::invoke(
        "hydration-diff-lens",
        &json!({
            "server_html": "<div id=\"r\">server</div>",
            "client_html": "<div id=\"r\">client</div>"
        }),
    )
    .expect("hydration-diff-lens");
    assert!(hyd["result"]["mismatch_count"].as_u64().unwrap() >= 1);

    let css = FeatureOrchestrator::invoke(
        "css-coverage-map",
        &json!({
            "rules": [
                { "selector": ".used", "used": true, "bytes": 40 },
                { "selector": ".dead", "used": false, "bytes": 60 }
            ]
        }),
    )
    .expect("css-coverage-map");
    assert_eq!(css["result"]["unused_bytes"], 60);

    let img = FeatureOrchestrator::invoke(
        "image-delivery-auditor",
        &json!({
            "images": [{
                "url": "https://cdn/x.png",
                "natural_w": 2000,
                "natural_h": 2000,
                "display_w": 100.0,
                "display_h": 100.0,
                "bytes": 900_000,
                "format": "png",
                "has_srcset": false,
                "loading": "eager"
            }]
        }),
    )
    .expect("image-delivery-auditor");
    assert!(img["result"]["oversized_count"].as_u64().unwrap() >= 1
        || !img["result"]["findings"].as_array().unwrap().is_empty());
}

#[test]
fn test_wave2_ai_invokes() {
    let cache = FeatureOrchestrator::invoke(
        "cache-hit-ledger",
        &json!({
            "events": [
                { "key": "p1", "hit": true, "tokens_saved": 100 },
                { "key": "p2", "hit": false, "tokens_saved": 0 }
            ]
        }),
    )
    .expect("cache-hit-ledger");
    assert_eq!(cache["result"]["hits"], 1);
    assert_eq!(cache["result"]["misses"], 1);
    assert!((cache["result"]["hit_rate"].as_f64().unwrap() - 0.5).abs() < 1e-9);

    let slo = FeatureOrchestrator::invoke(
        "streaming-slo-gates",
        &json!({
            "input": {
                "ttft_ms": 80,
                "tokens_per_sec": 40.0,
                "max_gap_ms": 20,
                "ttft_budget_ms": 200,
                "min_tokens_per_sec": 10.0,
                "max_gap_budget_ms": 100
            }
        }),
    )
    .expect("streaming-slo-gates");
    assert_eq!(slo["result"]["passed"], true);

    let mcp = FeatureOrchestrator::invoke(
        "mcp-traffic-lens",
        &json!({
            "messages": [
                { "method": "tools/list", "id": "1", "is_error": false, "bytes": 120, "latency_ms": 15 },
                { "method": "tools/call", "id": "2", "is_error": true, "bytes": 40, "latency_ms": 90 }
            ]
        }),
    )
    .expect("mcp-traffic-lens");
    assert_eq!(mcp["result"]["messages"], 2);
    assert_eq!(mcp["result"]["errors"], 1);
}

#[test]
fn test_wave2_perf_invokes() {
    let pool = FeatureOrchestrator::invoke(
        "conn-pool-inspector",
        &json!({
            "entries": [
                { "url": "https://a/x", "connection_id": "c1", "wait_ms": 2.0, "reused": true, "protocol": "h2" },
                { "url": "https://a/y", "connection_id": "c1", "wait_ms": 5.0, "reused": true, "protocol": "h2" },
                { "url": "https://b/z", "connection_id": "c2", "wait_ms": 40.0, "reused": false, "protocol": "http/1.1" }
            ]
        }),
    )
    .expect("conn-pool-inspector");
    assert_eq!(pool["result"]["requests"], 3);
    assert_eq!(pool["result"]["unique_connections"], 2);

    let freeze = FeatureOrchestrator::invoke(
        "freeze-resume-bench",
        &json!({
            "samples": [{
                "tab_id": "t1",
                "soft_freeze_ms": 12.0,
                "soft_unfreeze_ms": 18.0,
                "discard_ms": 30.0,
                "restore_ms": 45.0
            }]
        }),
    )
    .expect("freeze-resume-bench");
    assert_eq!(freeze["result"]["samples"], 1);
    assert!(freeze["result"]["avg_freeze_ms"].as_f64().unwrap() > 0.0);

    let soft = FeatureOrchestrator::invoke(
        "soft-nav-vitals",
        &json!({
            "records": [{
                "navigation_id": "n1",
                "url": "/app/dashboard",
                "lcp_ms": 900.0,
                "inp_ms": 40.0,
                "cls": 0.01,
                "ttfb_ms": 80.0
            }]
        }),
    )
    .expect("soft-nav-vitals");
    assert_eq!(soft["result"]["navigations"], 1);
    assert!((soft["result"]["avg_lcp_ms"].as_f64().unwrap() - 900.0).abs() < 1e-6);
}

#[test]
fn test_wave2_registry_still_144() {
    assert_eq!(FeatureId::ALL.len(), 144);
    assert_eq!(FeatureEngine::new().registry().len(), 144);
}

