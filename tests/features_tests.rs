//! Integration tests for the Thor Engineer Feature Suite (`src/features/`).
//!
//! Covers all registered features across the six specialist domains (frontend, backend,
//! security, performance, qa, ai) plus the Performance SLA subsystem. Each
//! persona group test asserts that every feature in that group is registered in
//! the [`FeatureRegistry`] and that its backing sub-engine logic executes
//! without panicking on representative inputs. Dedicated tests then drill into
//! the critical features called out for the Universal Evidence Engine.
//!
//! NOTE on the registry API: the shipped implementation models feature maturity
//! as [`FeatureStatus::{Stable, Beta, Experimental}`] and exposes the canonical
//! set via `FeatureRegistry::new().all()` (all are enabled/"implemented" by
//! default in [`FeatureEngine`]). These tests therefore assert
//! "N registered + enabled features, each with a valid status" as the concrete
//! realization of "all implemented features".

use std::collections::{BTreeMap, HashMap};

use thor::app_state::AppState;
use thor::features::{
    ai, backend, frontend, perf_features, qa, security, sla, FeatureCategory, FeatureEngine,
    FeatureId, FeatureRegistry, FeatureStatus,
};

// dx test-generator types reused by the security/qa export features.
use thor::dx::test_generator::{
    DomStep, ExportFormat, LocatorCandidate, NetworkTrace, RecordedSession, TestGenerator,
};
// load types backing the performance / SLA features.
use thor::load::{BrowserMetrics, LoadMetrics, PerformanceBudgets};

// ===========================================================================
// Helpers
// ===========================================================================

/// All feature ids belonging to a given category.
fn ids_in(category: FeatureCategory) -> Vec<FeatureId> {
    FeatureId::ALL
        .iter()
        .copied()
        .filter(|id| id.category() == category)
        .collect()
}

/// Assert that every feature id in a category is registered, enabled, and can be
/// invoked through the engine's runtime layer without panicking.
fn assert_group_registered_and_invocable(
    engine: &FeatureEngine,
    category: FeatureCategory,
    expected_count: usize,
) {
    let registry = engine.registry();
    let registered = registry.by_category(category);
    assert_eq!(
        registered.len(),
        expected_count,
        "{:?} should have {} registered features, got {}",
        category,
        expected_count,
        registered.len()
    );

    for id in ids_in(category) {
        // Registered in the registry with a matching category.
        let desc = registry
            .get(id)
            .unwrap_or_else(|| panic!("feature {:?} ({}) must be registered", id, id.slug()));
        assert_eq!(desc.category, category);
        assert_eq!(desc.slug, id.slug());
        assert_eq!(desc.name, id.name());

        // Enabled by default and invocable through the runtime tracker.
        assert!(engine.is_enabled(id), "{} should be enabled", id.slug());
        assert_eq!(
            engine.record_invocation(id),
            Some(1),
            "invoking {} must succeed",
            id.slug()
        );
    }
}

fn sample_vitals() -> BrowserMetrics {
    BrowserMetrics {
        lcp_ms: 2000.0,
        inp_ms: 150.0,
        cls: 0.05,
        ttfb_ms: 400.0,
        fcp_ms: 900.0,
        ..Default::default()
    }
}

fn sample_load_metrics() -> LoadMetrics {
    LoadMetrics {
        total_requests: 1000,
        successful_requests: 995,
        failed_requests: 5,
        total_bytes: 1_000_000,
        duration_secs: 10.0,
        rps: 100.0,
        min_latency_ms: 5.0,
        max_latency_ms: 900.0,
        avg_latency_ms: 120.0,
        p50_latency_ms: 100.0,
        p90_latency_ms: 250.0,
        p95_latency_ms: 300.0,
        p99_latency_ms: 800.0,
        status_codes: HashMap::new(),
        browser_vitals: None,
        transfer_bytes: None,
        decoded_bytes: None,
        lcp_media_bytes: None,
        soft_nav: None,
    }
}

// ===========================================================================
// 1. Registry-wide coverage — all registered features implemented
// ===========================================================================

#[test]
fn test_registry_has_all_features() {
    let engine = FeatureEngine::new();
    let n = FeatureId::ALL.len();
    assert!(n >= 84, "expected at least wave-2 QA expansion (84+), got {n}");
    assert_eq!(engine.registry().len(), n, "registry must hold all feature ids");

    let registry = FeatureRegistry::new();
    assert_eq!(registry.all().len(), n);
    assert!(!registry.is_empty());
}

#[test]
fn test_all_features_registered_enabled_and_have_status() {
    // Concrete realization of "all_implemented() returns N features with a
    // valid (implemented) status": every canonical feature is registered,
    // enabled by default, and carries a maturity status.
    let engine = FeatureEngine::new();
    let registry = engine.registry();

    let mut implemented = 0usize;
    for id in FeatureId::ALL {
        let desc = registry.get(id).expect("registered");
        // Valid maturity status == a real, shipped implementation.
        assert!(matches!(
            desc.status,
            FeatureStatus::Stable | FeatureStatus::Beta | FeatureStatus::Experimental
        ));
        assert!(engine.is_enabled(id), "{} enabled", id.slug());
        implemented += 1;
    }
    assert_eq!(implemented, FeatureId::ALL.len(), "all features must be implemented + enabled");
}

#[test]
fn test_no_duplicate_slugs_and_lookup_by_slug() {
    let registry = FeatureRegistry::new();
    let mut seen = std::collections::HashSet::new();
    for id in FeatureId::ALL {
        assert!(seen.insert(id.slug()), "duplicate slug: {}", id.slug());
        // Round-trip: slug -> descriptor -> id.
        assert_eq!(registry.by_slug(id.slug()).map(|d| d.id), Some(id));
    }
    assert_eq!(seen.len(), FeatureId::ALL.len());
    assert!(registry.by_slug("does-not-exist").is_none());
}

#[test]
fn test_category_counts_sum_to_registry() {
    let registry = FeatureRegistry::new();
    let frontend = registry.by_category(FeatureCategory::Frontend).len();
    let backend = registry.by_category(FeatureCategory::Backend).len();
    let sec = registry.by_category(FeatureCategory::Security).len();
    let perf = registry.by_category(FeatureCategory::Performance).len();
    let qa = registry.by_category(FeatureCategory::Qa).len();
    let ai = registry.by_category(FeatureCategory::Ai).len();

    assert!(frontend >= 11);
    assert!(backend >= 12);
    assert!(sec >= 12);
    assert!(perf >= 13);
    assert_eq!(qa, 24, "QA wave-1+2 must be 24");
    assert!(ai >= 12);
    assert_eq!(frontend + backend + sec + perf + qa + ai, FeatureId::ALL.len());
}

#[test]
fn test_status_report_and_json_shape() {
    let engine = FeatureEngine::new();
    let report = engine.status_report();
    assert_eq!(report.len(), FeatureId::ALL.len());
    assert!(report.iter().all(|r| r.enabled));

    let json = engine.status_report_json();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).expect("valid json");
    assert_eq!(parsed.len(), FeatureId::ALL.len());
    let first = &parsed[0];
    assert!(first.get("id").is_some());
    assert!(first.get("slug").is_some());
    assert!(first.get("category").is_some());
    assert!(first.get("status").is_some());
    assert!(first.get("enabled").is_some());
}

#[test]
fn test_feature_toggle_disables_invocation() {
    let engine = FeatureEngine::new();
    assert_eq!(engine.record_invocation(FeatureId::JwtDeep), Some(1));
    engine.set_enabled(FeatureId::JwtDeep, false);
    assert!(!engine.is_enabled(FeatureId::JwtDeep));
    assert_eq!(engine.record_invocation(FeatureId::JwtDeep), None);
    // Re-enable and confirm the counter resumes from its prior value.
    engine.set_enabled(FeatureId::JwtDeep, true);
    assert_eq!(engine.record_invocation(FeatureId::JwtDeep), Some(2));
}

#[test]
fn test_app_state_global_exposes_feature_engine() {
    let engine = AppState::global().features();
    assert_eq!(engine.registry().len(), FeatureId::ALL.len());
    // Sub-engines are reachable via the shared engine.
    assert!(engine
        .registry()
        .get(FeatureId::IdorMatrix)
        .is_some());
}

// ===========================================================================
// 2. Persona group tests — registration + execute without panic
// ===========================================================================

#[test]
fn test_frontend_group_registered_and_executes() {
    let engine = FeatureEngine::new();
    assert_group_registered_and_invocable(&engine, FeatureCategory::Frontend, 23);

    let fe = engine.frontend();
    // 1. Component Tree Lens
    let root = frontend::DomNode::new("div")
        .child(frontend::DomNode::new("app-header"))
        .child(frontend::DomNode::new("div").attr("data-reactroot", ""));
    assert!(fe.build_component_tree(&root).node_count >= 3);
    // 2. Responsive Matrix
    let vps = frontend::FrontendEngine::standard_viewports();
    assert_eq!(vps.len(), 4);
    let boxes = vec![frontend::ElementBox {
        selector: ".hero".into(),
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 40.0,
        font_size_px: 16.0,
    }];
    let _ = fe.responsive_matrix(&[(vps[0].clone(), boxes)]);
    // 3. CSS Cascade
    let decls = vec![
        frontend::CssDeclaration {
            selector: "div".into(),
            property: "color".into(),
            value: "red".into(),
            important: false,
            order: 0,
        },
        frontend::CssDeclaration {
            selector: "#main".into(),
            property: "color".into(),
            value: "blue".into(),
            important: false,
            order: 1,
        },
    ];
    assert!(!fe.resolve_cascade(&decls).is_empty());
    // 4. Design Tokens
    let _ = fe.extract_design_tokens(&decls);
    // 5. Accessibility Lens
    let _ = fe.accessibility_audit(&[frontend::DomNode::new("img")]);
    // 6. State Time-Travel
    fe.record_state("a", serde_json::json!({"n": 0}));
    fe.record_state("b", serde_json::json!({"n": 1}));
    assert_eq!(fe.state_diff(0, 1).len(), 1);
    // 7. CLS Heatmap
    let _ = fe.cls_heatmap(&[frontend::LayoutShiftEntry {
        node: "ad".into(),
        score: 0.2,
        had_recent_input: false,
    }]);
    // 8. Component Sandbox
    assert!(fe
        .component_sandbox("my-widget", &serde_json::json!({"a": 1}), "<span/>")
        .contains("my-widget"));
    // 9. Network<->Component binding
    let _ = fe.bind_network_to_components(
        &[frontend::RenderWindow { component: "Cmp".into(), start_ms: 0, end_ms: 100 }],
        &[frontend::NetworkEvent { url: "/api".into(), start_ms: 50, initiator_component: None }],
    );
    // 10. Font Auditor
    let _ = fe.audit_fonts(
        &[frontend::FontFace { family: "Inter".into(), weight: 400, font_display: None, bytes: 1000 }],
        &["Inter".to_string()],
    );
    // 11. Z-Index Visualizer
    let _ = fe.visualize_zindex(&[frontend::StackingElement {
        selector: ".modal".into(),
        z_index: Some(10),
        creates_stacking_context: true,
        position: "fixed".into(),
    }]);
}

#[test]
fn test_backend_group_registered_and_executes() {
    let engine = FeatureEngine::new();
    assert_group_registered_and_invocable(&engine, FeatureCategory::Backend, 24);

    let be = engine.backend();
    // 1. API Workbench (offline template resolution, no network)
    let mut vars = HashMap::new();
    vars.insert("id".to_string(), "42".to_string());
    let req = be.build_request(
        backend::ApiRequest { method: "GET".into(), url: "/u/{{id}}".into(), ..Default::default() },
        &vars,
    );
    assert_eq!(req.url, "/u/42");
    let _ = be.to_intercepted(&req, "r1");
    // 2. Auth Vault
    be.store_auth("api", backend::AuthScheme::Bearer { token: "abc".into() });
    assert!(be.auth_header("api").is_some());
    // 3. Contract Validation
    let schema = serde_json::json!({"type": "object", "required": ["id"]});
    assert!(be.validate_contract(&serde_json::json!({"id": 1}), &schema).is_empty());
    // 4. Webhook Sink
    be.record_webhook(backend::WebhookEvent {
        method: "POST".into(),
        path: "/hook".into(),
        headers: HashMap::new(),
        body: "{}".into(),
        received_ms: 1,
    });
    assert_eq!(be.webhooks().len(), 1);
    // 5. IDOR Matrix
    assert_eq!(
        be.analyze_idor(&[backend::IdorProbe {
            identity: "attacker".into(),
            resource: "/u/2".into(),
            is_owner: false,
            status: 200,
        }])
        .len(),
        1
    );
    // 6. Request Chaining
    let mut login = backend::ChainStep {
        name: "login".into(),
        request: backend::ApiRequest { method: "POST".into(), url: "/login".into(), ..Default::default() },
        captures: HashMap::new(),
    };
    login.captures.insert("token".into(), "data.token".into());
    let use_step = backend::ChainStep {
        name: "me".into(),
        request: backend::ApiRequest { method: "GET".into(), url: "/me?t={{token}}".into(), ..Default::default() },
        captures: HashMap::new(),
    };
    assert!(be.resolve_chain(&[login, use_step]).is_ok());
    // 7. gRPC / GraphQL
    assert_eq!(be.analyze_graphql("query Q { user { id } }").operation_type, "query");
    assert!(be.parse_grpc_path("/pkg.Svc/Method").is_some());
    // 8. Trace Adjacency
    let _ = be.build_trace_graph(&[backend::Span {
        span_id: "1".into(),
        parent_id: None,
        service: "gw".into(),
        name: "GET".into(),
        duration_ms: 10.0,
    }]);
    // 9. Mock / Stub
    be.add_mock(backend::MockRoute { method: "GET".into(), path_pattern: "/u/:id".into(), status: 200, body: "{}".into() });
    assert!(be.match_mock("GET", "/u/1").is_some());
    // 10. Chaos Injection (deterministic)
    be.add_chaos_rule(backend::ChaosRule { url_pattern: "api".into(), added_latency_ms: 10, error_rate: 1.0, error_status: 500 });
    assert_eq!(be.apply_chaos("https://api.x", "r1").forced_status, Some(500));
    // 11. Idempotency Auditor
    let _ = be.audit_idempotency(&[
        backend::IdempotencyEvent { method: "POST".into(), url: "/pay".into(), idempotency_key: None },
        backend::IdempotencyEvent { method: "POST".into(), url: "/pay".into(), idempotency_key: None },
    ]);
    // 12. DB Adjacency
    let _ = be.analyze_db(&["SELECT * FROM users JOIN orders ON users.id = orders.user_id".to_string()]);
}

#[test]
fn test_security_group_registered_and_executes() {
    let engine = FeatureEngine::new();
    assert_group_registered_and_invocable(&engine, FeatureCategory::Security, 24);

    let se = engine.security();
    // 1. Authorization Matrix
    let _ = se.build_authz_matrix(&[security::AuthzProbe {
        role: "user".into(),
        endpoint: "/admin".into(),
        should_allow: false,
        status: 200,
    }]);
    // 2. Response Diff
    let _ = se.diff_responses(
        &security::HttpSnapshot { status: 200, headers: BTreeMap::new(), body: "a".into() },
        &security::HttpSnapshot { status: 403, headers: BTreeMap::new(), body: "b".into() },
    );
    // 3. Header Analyzer
    assert_eq!(se.analyze_headers(&BTreeMap::new()).grade, "F");
    // 4. JWT Deep
    assert!(se.inspect_jwt("eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0.x", 0).is_ok());
    // 5. Promote to Finding
    assert!(se
        .promote_to_finding("bug", "https://x", vec![NetworkTrace {
            method: "GET".into(),
            url: "https://x/y".into(),
            headers: HashMap::new(),
            body: None,
            status: 500,
        }])
        .contains("# Finding"));
    // 6. Surface Crawler
    assert!(!se
        .crawl_surface(&[("GET".into(), "/api/users/1".into())])
        .is_empty());
    // 7. Storage Auditor
    let _ = se.audit_storage(&[security::StorageItem {
        store: "localStorage".into(),
        key: "auth_token".into(),
        value: "x".into(),
        http_only: false,
        secure: false,
        same_site: None,
    }]);
    // 8. Fuzz Repeater
    assert!(!se.build_fuzz_cases("/q?x=FUZZ", security::FuzzCategory::Xss).is_empty());
    // 9. CSRF Analyzer
    let _ = se.analyze_csrf(&security::CsrfContext {
        method: "POST".into(),
        has_csrf_token: false,
        cookie_same_site: None,
        uses_cookie_auth: true,
        content_type: Some("application/x-www-form-urlencoded".into()),
    });
    // 10. Sink / Pollution
    assert!(!se
        .detect_sinks(&serde_json::json!({"__proto__": {}}), Some("eval(x)"))
        .is_empty());
    // 11. TLS Inspector
    let _ = se.inspect_tls(&security::TlsInfo {
        protocol: "TLS1.3".into(),
        cipher: "TLS_AES_256_GCM_SHA384".into(),
        cert_days_remaining: 90,
        self_signed: false,
        hostname_matches: true,
    });
    // 12. OOB Listener
    let token = se.new_oob_token();
    se.record_oob(security::OobInteraction {
        token: token.clone(),
        protocol: "dns".into(),
        source_ip: "1.2.3.4".into(),
        received_ms: 1,
        detail: "A".into(),
    });
    assert_eq!(se.oob_interactions(&token).len(), 1);
}

#[test]
fn test_performance_group_registered_and_executes() {
    let engine = FeatureEngine::new();
    assert_group_registered_and_invocable(&engine, FeatureCategory::Performance, 25);

    let pe = engine.perf();
    let budgets = PerformanceBudgets::core_web_vitals();
    // 1. Perf Budgets
    let _ = pe.check_budgets(&budgets, Some(&sample_vitals()), Some(&sample_load_metrics()));
    // 2. Trace Diff
    let mut before = BTreeMap::new();
    before.insert("lcp".to_string(), 1000.0);
    let mut after = BTreeMap::new();
    after.insert("lcp".to_string(), 1500.0);
    assert!(!pe.diff_traces(&before, &after, 10.0).is_empty());
    // 3. Waterfall Forensics
    let res = vec![perf_features::ResourceTiming {
        url: "https://x/app.js".into(),
        start_ms: 0.0,
        dns_ms: 1.0,
        connect_ms: 1.0,
        ttfb_ms: 10.0,
        download_ms: 5.0,
        render_blocking: true,
        bytes: 1000,
    }];
    let _ = pe.analyze_waterfall(&res);
    // 4. Memory Leak Hunter
    let samples: Vec<perf_features::HeapSample> = (0..10)
        .map(|i| perf_features::HeapSample { timestamp_ms: i * 1000, used_bytes: 10_000_000 + i * 5_000_000 })
        .collect();
    assert!(pe.detect_leak(&samples).leaking);
    // 5. Synthetic Monitor
    pe.record_synthetic(perf_features::SyntheticCheck { target: "api".into(), success: true, latency_ms: 100.0, timestamp_ms: 1 });
    assert_eq!(pe.synthetic_summary("api").checks, 1);
    // 6. RUM vs Lab
    let _ = pe.rum_vs_lab(&sample_vitals(), &sample_vitals());
    // 7. HDR Histogram Helper
    let hist = pe.histogram_from_samples(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    assert!(!pe.percentile_table(&hist).is_empty());
    // 8. INP Attribution
    let _ = pe.attribute_inp(&[perf_features::InteractionEvent {
        target: "#b".into(),
        input_delay_ms: 10.0,
        processing_ms: 20.0,
        presentation_ms: 5.0,
    }]);
    // 9. CDN Cache
    let _ = pe.analyze_cache(&[perf_features::CacheEntry {
        url: "/a.js".into(),
        cache_control: Some("max-age=3600".into()),
        age_secs: Some(100),
        x_cache: Some("HIT".into()),
    }]);
    // 10. Vitals Under Load
    assert_eq!(pe.vitals_knee(&[(1, 1500.0), (50, 2600.0)], 2500.0), Some(50));
    // 11. Flame Graph
    assert_eq!(pe.build_flame_graph(&["main;a 10".to_string()]).value, 10);
    // 12. Third-Party Attribution
    let _ = pe.attribute_third_party("https://mysite.com", &res);
    // 13–24 wave-2 performance engines
    let _ = pe.lcp_element_forensics(&perf_features::LcpElementPayload {
        url: Some("https://cdn/hero.webp".into()),
        element_type: Some("image".into()),
        size_bytes: Some(40_000),
        discover_ms: Some(100.0),
        load_ms: Some(800.0),
        render_ms: Some(1800.0),
        ..Default::default()
    });
    let _ = pe.long_task_radar(&[perf_features::LongTaskEvent {
        duration_ms: 120.0,
        start_ms: 0.0,
        script_url: Some("app.js".into()),
        name: Some("script".into()),
        inp_overlap: true,
    }]);
    let _ = pe.main_thread_block_map(&[perf_features::TraceCategorySlice {
        category: "v8.execute".into(),
        duration_ms: 200.0,
    }]);
    let _ = pe.bfcache_probe(&[perf_features::BfCacheSample {
        restored: true,
        restore_ms: Some(10.0),
        not_restored_reasons: vec![],
    }]);
    let _ = pe.freeze_resume_bench(&[perf_features::FreezeTimingSample {
        tab_id: "t1".into(),
        soft_freeze_ms: 5.0,
        soft_unfreeze_ms: 6.0,
        discard_ms: None,
        restore_ms: None,
    }]);
    let _ = pe
        .soft_nav_vitals(&[perf_features::SoftNavRecord {
            navigation_id: "1".into(),
            url: "/a".into(),
            lcp_ms: 900.0,
            inp_ms: 40.0,
            cls: 0.01,
            ttfb_ms: 0.0,
        }])
        .expect("soft-nav");
    let _ = pe.speculation_lab(&[perf_features::SpeculationTrial {
        url: "/".into(),
        cold_nav_ms: 1000.0,
        prerender_nav_ms: Some(200.0),
        prefetch_nav_ms: None,
        rule_type: Some("prerender".into()),
    }]);
    let _ = pe.conn_pool_inspect(&[perf_features::ConnPoolEntry {
        url: "/".into(),
        connection_id: "c1".into(),
        wait_ms: 1.0,
        reused: true,
        protocol: Some("h3".into()),
    }]);
    let _ = pe.http3_path_audit(&[perf_features::Http3PathSample {
        url: "/".into(),
        alpn: Some("h3".into()),
        ttfb_ms: 40.0,
        fallback_from: None,
        fallback_cost_ms: None,
    }]);
    let _ = pe.compression_budget(
        &[perf_features::CompressionEntry {
            url: "/a.js".into(),
            transfer_bytes: 10_000,
            decoded_bytes: 40_000,
            content_encoding: Some("br".into()),
            content_type: Some("application/javascript".into()),
        }],
        5.0,
    );
    let _ = pe.media_weight_gate(
        &[perf_features::MediaResource {
            url: "hero.webp".into(),
            bytes: 100_000,
            media_type: "image".into(),
            contributes_to_lcp: true,
            lcp_ms: Some(1800.0),
        }],
        500_000,
        2_000_000,
        300_000,
    );
    let _ = pe.continuous_profile(&[perf_features::StackSample {
        stack: vec!["main".into(), "render".into()],
        weight: 3,
    }]);
    // Performance SLA
    let sla_engine = engine.sla();
    let report = sla_engine.check_run(&sla::SlaEngine::web_performance_sla(), None, Some(&sample_vitals()));
    assert!(report.overall_met);
}

#[test]
fn test_qa_group_registered_and_executes() {
    let engine = FeatureEngine::new();
    assert_group_registered_and_invocable(&engine, FeatureCategory::Qa, 24);

    let qe = engine.qa();
    // 1. Resilient Locators
    assert!(!qe
        .rank_locators(&qa::ElementSignals { test_id: Some("submit".into()), ..Default::default() })
        .is_empty());
    // 2. Flake Radar
    assert_eq!(qe.flake_radar("t", &[true, false, true, false]).classification, "flaky");
    // 3. Visual Regression
    let base = vec![10u8; 64];
    assert!(qe.visual_regression(&base, &base, 0.01).passed);
    // 4. Identity Fixtures
    let mut fx = qa::IdentityFixture { name: "admin".into(), role: "admin".into(), ..Default::default() };
    fx.cookies.insert("session".into(), "abc".into());
    qe.add_fixture(fx);
    assert!(qe.fixture_setup_script("admin").is_some());
    // 5. Data-Driven Scenarios
    let mut row = HashMap::new();
    row.insert("user".to_string(), "alice".to_string());
    assert_eq!(qe.expand_data_driven("as {{user}}", &[row]), vec!["as alice"]);
    // 6. Real Vitals Gates
    let bad = BrowserMetrics { lcp_ms: 9000.0, ..Default::default() };
    assert!(!qe.vitals_gate(&bad, &PerformanceBudgets::core_web_vitals()).passed);
    // 7. Contract Testing
    let interactions = vec![qa::ContractInteraction {
        description: "get".into(),
        method: "GET".into(),
        path: "/u/1".into(),
        expected_status: 200,
        response_schema: None,
    }];
    let actuals = vec![qa::ActualInteraction { method: "GET".into(), path: "/u/1".into(), status: 200, body: None }];
    assert!(qe.verify_contract(&interactions, &actuals).satisfied);
    // 8. Deflaked Export + 9. CI Bundle
    let mut session = RecordedSession::new("Flow", "https://x");
    session.add_trace(NetworkTrace { method: "GET".into(), url: "https://x/a".into(), headers: HashMap::new(), body: None, status: 200 });
    assert!(!qe.export_deflaked(&session).contains("waitForTimeout"));
    assert!(qe.export_ci_bundle(&session).contains("playwright"));
    // 10. Time-Travel Trace
    qe.record_trace_frame("login", "fill", "fill", "<input>");
    assert_eq!(qe.trace_frames("login").len(), 1);
    // 11. Determinism Harness
    assert!(qe.determinism(&["a".into(), "a".into()]).deterministic);
    // 12. Authz Matrix Tests
    let cases = qe.generate_authz_tests(&[("admin".to_string(), "/admin".to_string(), true)]);
    assert!(qe.authz_tests_to_playwright("https://x", &cases).contains("toBeLessThan(300)"));

    // Wave 2 smokes
    let radar = qe.flake_radar("t", &[true, false, true]);
    let _ = qe.triage_flake(
        "t",
        &radar,
        &qa::FlakeVector {
            timing_cv: 0.8,
            ..Default::default()
        },
        &[],
    );
    let _ = qe.evaluate_quarantine(&qa::QuarantineInput {
        test: "t".into(),
        flake_rate: 0.5,
        consecutive_fails: 3,
        days_in_quarantine: 0.0,
        green_streak: 0,
        currently_quarantined: false,
    });
    let _ = qe.contract_drift_gate(&interactions, &actuals);
    let _ = qe.build_hermetic_fixture_pack(
        "pack",
        qa::IdentityFixture {
            name: "u".into(),
            role: "user".into(),
            ..Default::default()
        },
        vec![],
        vec![],
    );
    let _ = qe.generate_a11y_spec(
        "home",
        &[qa::A11yViolation {
            rule_id: "label".into(),
            selector: "input".into(),
            impact: "serious".into(),
            message: "missing label".into(),
        }],
    );
    let _ = qe.snapshot_strategy_pack("s", &[1, 2, 3], "<div/>", "a", "{}");
    let _ = qe.coverage_adjacency(&[], &[]);
    let _ = qe.plan_parallel_shards(
        &[qa::ShardScenario {
            name: "a".into(),
            duration_ms: 10.0,
            identity: "x".into(),
        }],
        1,
    );
    let _ = qe.mine_trace_properties(&[]);
    let _ = qe.runtime_self_heal(
        &qa::ElementSignals {
            test_id: Some("x".into()),
            ..Default::default()
        },
        &[],
    );
    let _ = qe.synthesize_oracles(&qa::OracleObservation {
        status: Some(200),
        ..Default::default()
    });
    let _ = qe.mutation_hook_bridge(&[]);
}

#[test]
fn test_ai_group_registered_and_executes() {
    let engine = FeatureEngine::new();
    assert_group_registered_and_invocable(&engine, FeatureCategory::Ai, 24);

    let ae = engine.ai();
    // 1. LLM Inspector
    let body = serde_json::json!({
        "model": "gpt-4o",
        "stream": true,
        "messages": [{"role": "user", "content": "hi there"}]
    });
    assert_eq!(ae.inspect_request(&body).model, "gpt-4o");
    // 2. Streaming Waterfall
    let chunks = vec![
        ai::StreamChunk { raw: "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n".into(), arrival_ms: 100 },
        ai::StreamChunk { raw: "data: [DONE]\n\n".into(), arrival_ms: 150 },
    ];
    assert!(ae.streaming_waterfall(&chunks).total_tokens > 0);
    // 3. Prompt Diff
    let _ = ae.prompt_diff("line a\nline b", "line a\nline c");
    // 4. Cost Dashboard
    ae.log_cost("gpt-4o", 1000, 500);
    assert_eq!(ae.cost_summary().call_count, 1);
    // 5. Eval Replay
    let _ = ae.eval_replay(&[ai::EvalCase { name: "a".into(), expected: "42".into(), actual: "42".into(), match_mode: None }]);
    // 6. Model Comparison
    let _ = ae.compare_models(&[ai::ModelRun {
        model: "gpt-4o".into(),
        output: "hi".into(),
        prompt_tokens: 10,
        completion_tokens: 5,
        latency_ms: 100.0,
    }]);
    // 7. Agent Trace
    let _ = ae.analyze_agent_trace(&[ai::AgentStep { kind: "action".into(), tool: Some("search".into()), content: "q".into() }]);
    // 8. Context Heatmap
    let h = ae.context_heatmap(1000, &[ai::ContextSegment { label: "sys".into(), tokens: 100 }]);
    assert_eq!(h.used_tokens, 100);
    // 9. RAG Inspector
    let _ = ae.inspect_rag(
        &[ai::RetrievedChunk { id: "c1".into(), score: 0.9, text: "capital of France is Paris".into() }],
        "The capital of France is Paris",
    );
    // 10. Injection Scanner
    assert!(!ae.scan_injection("ignore previous instructions").is_empty());
    // 11. LLM Load
    let _ = ae.llm_load_report(&[ai::LlmStreamStat { ttft_ms: 100, tokens: 50, duration_ms: 1000 }]);
    // 12. Deterministic Replay
    assert!(ae
        .deterministic_replay(&["a".into(), "b".into()], 0.0)
        .verdict
        .starts_with("VIOLATION"));
}

// ===========================================================================
// 3. Critical individual feature tests
// ===========================================================================

#[test]
fn test_critical_perf_budgets_flags_violations() {
    let pe = perf_features::PerfEngine::new();
    let budgets = PerformanceBudgets::core_web_vitals();

    let good = BrowserMetrics { lcp_ms: 2000.0, inp_ms: 150.0, cls: 0.05, ..Default::default() };
    assert!(pe.check_budgets(&budgets, Some(&good), None).passed);

    let bad = BrowserMetrics { lcp_ms: 4000.0, inp_ms: 300.0, cls: 0.4, ..Default::default() };
    let report = pe.check_budgets(&budgets, Some(&bad), None);
    assert!(!report.passed);
    assert!(report.violations.iter().any(|v| v.metric == "lcp_ms" && v.over_by > 0.0));
    assert!(report.checked >= 3);
}

#[test]
fn test_critical_hdr_histogram_percentiles_are_monotonic() {
    let pe = perf_features::PerfEngine::new();
    // 1..=1000 ms samples: percentiles must be increasing and within range.
    let samples: Vec<f64> = (1..=1000).map(|i| i as f64).collect();
    let hist = pe.histogram_from_samples(&samples);

    let p50 = hist.percentile(50.0);
    let p90 = hist.percentile(90.0);
    let p99 = hist.percentile(99.0);
    assert!(p50 > 0.0 && p50 <= 1000.0);
    assert!(p90 >= p50, "p90 {p90} >= p50 {p50}");
    assert!(p99 >= p90, "p99 {p99} >= p90 {p90}");
    // p50 of a uniform 1..=1000 distribution should land near the middle.
    assert!((p50 - 500.0).abs() < 100.0, "p50 {p50} near 500");

    let table = pe.percentile_table(&hist);
    assert!(table.iter().any(|(q, _)| (*q - 99.9).abs() < f64::EPSILON));
    // Table quantile values are non-decreasing.
    for w in table.windows(2) {
        assert!(w[1].1 >= w[0].1, "percentile table not monotonic: {:?}", table);
    }
}

#[test]
fn test_critical_deflaked_playwright_export_has_no_wait_for_timeout() {
    let qe = qa::QaEngine::new();
    let mut session = RecordedSession::new("Login", "https://app.example.com");
    // A brittle fixed "wait" step must be lowered to a web-first wait, never a
    // fixed waitForTimeout.
    session.add_dom_step(DomStep::new("fill", "#email").with_value("a@b.com"));
    session.add_dom_step(DomStep::new("wait", "3000"));
    session.add_dom_step(DomStep::new("click", "[data-testid=\"submit\"]"));

    let ts = qe.export_deflaked(&session);
    assert!(!ts.contains("waitForTimeout"), "deflaked export must not use waitForTimeout");
    assert!(ts.contains("await expect("), "deflaked export must use web-first assertions");
    assert!(ts.contains("networkidle") || ts.contains("toBeVisible"));

    // The brittle-pattern detector should flag a raw waitForTimeout source.
    let brittle = qe.detect_brittle_patterns("await page.waitForTimeout(1000);\nawait page.locator('div:nth-child(3)').click();");
    assert!(brittle.iter().any(|i| i.contains("waitForTimeout")));
    assert!(brittle.iter().any(|i| i.contains("nth-child")));
}

#[test]
fn test_critical_authz_matrix_detects_escalation_and_over_restriction() {
    let se = security::SecurityEngine::new();
    let report = se.build_authz_matrix(&[
        // Non-privileged role reaching a privileged endpoint => escalation.
        security::AuthzProbe { role: "user".into(), endpoint: "/admin".into(), should_allow: false, status: 200 },
        // Legit admin denied => over-restriction.
        security::AuthzProbe { role: "admin".into(), endpoint: "/admin".into(), should_allow: true, status: 403 },
        // Correct: user allowed on their own resource.
        security::AuthzProbe { role: "user".into(), endpoint: "/me".into(), should_allow: true, status: 200 },
    ]);
    assert_eq!(report.escalations, 1);
    assert_eq!(report.over_restrictions, 1);
    assert!(report.cells.iter().any(|c| c.verdict == "PRIVILEGE-ESCALATION"));
    assert!(report.cells.iter().any(|c| c.verdict == "OVER-RESTRICTED"));
    assert!(report.cells.iter().any(|c| c.verdict == "OK"));
}

#[test]
fn test_critical_llm_cost_dashboard_aggregates_per_model() {
    let ae = ai::AiEngine::new();
    ae.log_cost("gpt-4o", 1000, 500);
    ae.log_cost("gpt-4o", 2000, 1000);
    ae.log_cost("claude-3.5-sonnet", 500, 500);

    let summary = ae.cost_summary();
    assert_eq!(summary.call_count, 3);
    assert_eq!(summary.per_model.get("gpt-4o").unwrap().calls, 2);
    assert_eq!(summary.per_model.get("gpt-4o").unwrap().tokens, 4500);
    assert!(summary.per_model.contains_key("claude-3.5-sonnet"));
    assert!(summary.total_cost_usd > 0.0);
    assert_eq!(summary.total_tokens, 5500);

    ae.reset_costs();
    assert_eq!(ae.cost_summary().call_count, 0);
}

#[test]
fn test_critical_jwt_deep_analysis_flags_alg_none_and_missing_exp() {
    let se = security::SecurityEngine::new();
    // header {"alg":"none"} . payload {"sub":"1"} . sig
    let report = se
        .inspect_jwt("eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0.x", 0)
        .expect("jwt decodes");
    assert_eq!(report.algorithm.to_lowercase(), "none");
    assert!(report.issues.iter().any(|i| i.contains("alg=none")));
    assert!(report.issues.iter().any(|i| i.contains("missing 'exp'")));
}

#[test]
fn test_critical_resilient_locators_prefer_test_id_and_role() {
    let qe = qa::QaEngine::new();
    let ranked = qe.rank_locators(&qa::ElementSignals {
        test_id: Some("submit-btn".into()),
        role: Some("button".into()),
        accessible_name: Some("Submit".into()),
        label: Some("Submit form".into()),
        text: Some("Submit".into()),
        id: Some("submit".into()),
        css: Some(".btn.primary".into()),
    });
    // Highest resilience strategy first.
    assert_eq!(ranked[0].strategy, "test-id");
    assert!(ranked[0].expression.contains("getByTestId"));
    assert_eq!(ranked[0].resilience_score, 100);
    // Ranking is sorted by descending resilience score.
    for w in ranked.windows(2) {
        assert!(w[0].resilience_score >= w[1].resilience_score);
    }
    assert!(ranked.iter().any(|r| r.strategy == "role"));

    // The underlying LocatorCandidate emits Playwright role/test-id locators and
    // infers a test-id from a raw selector.
    let cand = LocatorCandidate::from_selector("[data-testid=\"login\"]");
    assert_eq!(cand.to_playwright().as_deref(), Some("page.getByTestId('login')"));
    let role_only = LocatorCandidate { role: Some("button".into()), name: Some("Save".into()), ..Default::default() };
    assert_eq!(
        role_only.to_playwright().as_deref(),
        Some("page.getByRole('button', { name: 'Save' })")
    );
}

// ===========================================================================
// 4. Performance SLA subsystem
// ===========================================================================

#[test]
fn test_performance_sla_pass_fail_and_error_budget() {
    let engine = sla::SlaEngine::new();
    let sla = sla::SlaEngine::web_performance_sla();

    let good = BrowserMetrics { lcp_ms: 2000.0, inp_ms: 150.0, cls: 0.05, ttfb_ms: 500.0, ..Default::default() };
    let report = engine.check_run(&sla, None, Some(&good));
    assert!(report.overall_met);
    assert_eq!(report.met_count, report.total);

    let bad = BrowserMetrics { lcp_ms: 3200.0, inp_ms: 150.0, cls: 0.05, ttfb_ms: 500.0, ..Default::default() };
    let report = engine.check_run(&sla, None, Some(&bad));
    assert!(!report.overall_met);
    assert!(report.results.iter().any(|r| r.metric == "lcp_ms" && !r.met));

    // Error budget maths: 99.9% target, observed 99.95% -> ~half budget left.
    let remaining = sla::SlaEngine::error_budget_remaining(99.9, 99.95);
    assert!((remaining - 0.5).abs() < 0.05);
    assert!(sla::SlaEngine::error_budget_remaining(99.9, 99.0) < 0.0);

    // Reliability SLA derived from a real load run.
    let reliability = engine.check_run(&sla::SlaEngine::reliability_sla(), Some(&sample_load_metrics()), None);
    assert_eq!(reliability.total, 4);
}

// ===========================================================================
// 5. Universal 'Promote to Test' export formats used by the feature suite
// ===========================================================================

#[test]
fn test_promote_to_test_export_formats() {
    let mut session = RecordedSession::new("Checkout", "https://shop.example.com");
    session.add_dom_step(
        DomStep::new("fill", "#qty")
            .with_value("2")
            .with_locator(LocatorCandidate { test_id: Some("qty".into()), ..Default::default() }),
    );
    session.add_trace(NetworkTrace {
        method: "POST".into(),
        url: "https://shop.example.com/cart".into(),
        headers: HashMap::new(),
        body: Some("{}".into()),
        status: 500,
    });

    // Playwright uses resilient locators + web-first assertions.
    let pw = TestGenerator::promote_to_test(&session, ExportFormat::PlaywrightTs);
    assert!(pw.contains("page.getByTestId('qty')"));
    assert!(!pw.contains("waitForTimeout"));

    assert!(TestGenerator::promote_to_test(&session, ExportFormat::K6Js).contains("http.request('POST'"));
    assert!(TestGenerator::promote_to_test(&session, ExportFormat::Curl).contains("curl -i -X POST"));
    assert!(TestGenerator::promote_to_test(&session, ExportFormat::EvalHarness).contains("expectStatus: 500"));
    assert!(TestGenerator::promote_to_test(&session, ExportFormat::CiBundle).contains("playwright install"));

    let finding = TestGenerator::promote_to_test(&session, ExportFormat::Finding);
    assert!(finding.contains("# Finding: Checkout"));
    assert!(finding.contains("500"));
}
