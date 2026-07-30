//! Wave-4 P0-12 regression pack for the Thor Engineer Feature Suite.
//!
//! Covers consent/hydration/SSE honesty contracts landed across coder + perf
//! waves while keeping the registry pinned at 144.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde_json::json;

use thor::features::ai::AiEngine;
use thor::features::backend::{BackendEngine, HeaderContract};
use thor::features::cdp_bridge::CdpBridge;
use thor::features::orchestrator::FeatureOrchestrator;
use thor::features::perf_features::{PerfEngine, ResourceBudgetInputs, SoftNavRecord};
use thor::features::qa::{self, QuarantineInput};
use thor::features::security::{SecurityEngine, SessionFixationProbe};
use thor::features::{FeatureEngine, FeatureId};
use thor::load::{BrowserMetrics, PerformanceBudgets};

#[test]
fn test_wave4_registry_still_144() {
    assert_eq!(FeatureId::ALL.len(), 144);
    assert_eq!(FeatureEngine::new().registry().len(), 144);
}

#[test]
fn test_session_fixation_requires_target_url() {
    let e = SecurityEngine::new();
    let missing = e.session_fixation_lab(&SessionFixationProbe {
        pre_auth_session: "sid".into(),
        post_auth_session: "sid".into(),
        login_status: 200,
        target_url: String::new(),
        lab_consent: false,
    });
    let err = missing.expect_err("empty target_url must Err");
    assert!(
        err.contains("requires target_url") || err.contains("target_url"),
        "err={err}"
    );

    let orch = FeatureOrchestrator::invoke(
        "session-fixation-lab",
        &json!({
            "probe": {
                "pre_auth_session": "S",
                "post_auth_session": "S",
                "login_status": 200,
                "target_url": ""
            }
        }),
    );
    let orch_err = orch.expect_err("orch missing target_url");
    assert!(
        orch_err.contains("target_url"),
        "orch err={orch_err}"
    );

    let ok = e
        .session_fixation_lab(&SessionFixationProbe {
            pre_auth_session: "sid".into(),
            post_auth_session: "sid".into(),
            login_status: 200,
            target_url: "http://127.0.0.1/login".into(),
            lab_consent: false,
        })
        .expect("local target ok");
    assert!(ok.vulnerable);
}

#[test]
fn test_soft_nav_refuses_all_zero() {
    let e = PerfEngine::new();
    let err = e
        .soft_nav_vitals(&[SoftNavRecord {
            navigation_id: "n1".into(),
            url: "/app".into(),
            lcp_ms: 0.0,
            inp_ms: 0.0,
            cls: 0.0,
            ttfb_ms: 5.0,
        }])
        .expect_err("all-zero soft-nav must refuse");
    assert!(
        err.contains("refused") && err.contains("lcp=inp=cls=0"),
        "err={err}"
    );

    let orch_err = FeatureOrchestrator::invoke(
        "soft-nav-vitals",
        &json!({
            "records": [{
                "navigation_id": "n1",
                "url": "/x",
                "lcp_ms": 0.0,
                "inp_ms": 0.0,
                "cls": 0.0,
                "ttfb_ms": 0.0
            }]
        }),
    )
    .expect_err("orch soft-nav all-zero");
    assert!(orch_err.contains("refused"), "orch err={orch_err}");

    let good = e
        .soft_nav_vitals(&[SoftNavRecord {
            navigation_id: "n2".into(),
            url: "/ok".into(),
            lcp_ms: 400.0,
            inp_ms: 20.0,
            cls: 0.01,
            ttfb_ms: 10.0,
        }])
        .expect("non-zero soft-nav");
    assert_eq!(good.navigations, 1);
}

#[test]
fn test_quarantine_hydrate_round_trip() {
    let tmp = std::env::temp_dir().join(format!(
        "thor-wave4-q-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::env::set_var("THOR_DATA_DIR", &tmp);

    let writer = qa::QaEngine::new();
    let d = writer.evaluate_quarantine(&QuarantineInput {
        test: "wave4.spec.ts".into(),
        flake_rate: 0.7,
        consecutive_fails: 5,
        days_in_quarantine: 0.0,
        green_streak: 0,
        currently_quarantined: false,
    });
    assert_eq!(d.action, "quarantine");
    let path = writer.persist_quarantine().expect("persist");
    assert!(path.exists());
    let yaml = qa::QaEngine::load_quarantine_yaml().expect("load yaml");
    assert!(yaml.contains("wave4.spec.ts"));

    // Fresh engine must hydrate from disk.
    let reader = qa::QaEngine::new();
    let status = reader
        .quarantine_status("wave4.spec.ts")
        .expect("hydrated entry");
    assert_eq!(status.action, "quarantine");
    assert!(reader.ci_quarantine_filter().contains("wave4"));

    std::env::remove_var("THOR_DATA_DIR");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_sse_tool_result_merge_honesty() {
    let call_only = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"lookup","arguments":"{\"id\":1}"}}]}}],"created_ms":1000}
data: [DONE]
"#;
    let only = AiEngine::tool_call_events_from_sse(call_only);
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].name, "lookup");
    assert!(
        !only[0].success,
        "call-only must not report success"
    );

    let call_and_result = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"lookup","arguments":"{}"}}]}}],"created_ms":1000}
data: {"type":"tool_result","tool_call_id":"call_1","content":"ok","created_ms":1080}
data: [DONE]
"#;
    let merged = AiEngine::tool_call_events_from_sse(call_and_result);
    assert_eq!(merged.len(), 1);
    assert!(merged[0].success, "call+tool_result must succeed");
    assert_eq!(merged[0].latency_ms, 80);

    let orch = FeatureOrchestrator::invoke(
        "tool-call-forensics",
        &json!({ "sse": call_only }),
    )
    .expect("tool-call-forensics call-only");
    assert_eq!(orch["result"]["failures"], 1);
}

#[test]
fn test_header_fuzz_execute_optional_when_wired() {
    let be = BackendEngine::new();
    let mut base = BTreeMap::new();
    base.insert("authorization".into(), "Bearer t".into());
    base.insert("content-type".into(), "application/json".into());
    let contract = HeaderContract {
        required: vec!["authorization".into()],
        forbidden: vec!["x-debug".into()],
        patterns: BTreeMap::new(),
    };

    let dry = be.header_contract_fuzzer(&base, &contract);
    assert!(!dry.executed);
    assert!(!dry.cases.is_empty());

    // execute:true without url → Err (wired path).
    let missing_url = be.header_contract_fuzzer_gated(&base, &contract, None, true, false);
    assert!(missing_url.is_err());

    // Local loopback execute path.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let flag = running.clone();
    let handle = thread::spawn(move || {
        listener.set_nonblocking(true).ok();
        while flag.load(Ordering::SeqCst) {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
    });
    let url = format!("http://{addr}/fuzz");
    let live = be
        .header_contract_fuzzer_gated(&base, &contract, Some(&url), true, false)
        .expect("local execute");
    assert!(live.executed);
    assert!(live.cases.iter().any(|c| c.status.is_some() || c.error.is_some()));

    let orch = FeatureOrchestrator::invoke(
        "header-contract-fuzzer",
        &json!({
            "base": { "authorization": "Bearer t" },
            "contract": { "required": ["authorization"], "forbidden": [], "patterns": {} },
            "execute": true,
            "url": url,
            "lab_consent": false
        }),
    )
    .expect("orch header fuzz execute");
    assert_eq!(orch["result"]["executed"], true);

    running.store(false, Ordering::SeqCst);
    let _ = handle.join();
}

#[test]
fn test_from_active_tab_errs_without_chromium() {
    // Ensure no evaluator is registered (fresh process / cleared).
    let err = CdpBridge::evaluate_on_active("1+1").expect_err("no chromium");
    assert!(
        err.contains("Chromium") || err.contains("from_active_tab") || err.contains("unavailable"),
        "err={err}"
    );

    let orch = FeatureOrchestrator::invoke(
        "dom-snapshot-capture",
        &json!({ "from_active_tab": true }),
    );
    let orch_err = orch.expect_err("from_active_tab without runtime");
    assert!(
        orch_err.contains("Chromium")
            || orch_err.contains("from_active_tab")
            || orch_err.contains("unavailable")
            || orch_err.contains("active"),
        "orch err={orch_err}"
    );
}

#[test]
fn test_budgets_check_uses_real_bytes_when_provided() {
    let e = PerfEngine::new();
    let budgets = PerformanceBudgets {
        max_transfer_bytes: Some(100_000),
        max_decoded_inflation_ratio: Some(2.0),
        max_lcp_media_bytes: Some(50_000),
        ..PerformanceBudgets::default()
    };
    let vitals = BrowserMetrics {
        lcp_ms: 1000.0,
        ..Default::default()
    };

    let over = e.check_budgets_with_resources(
        &budgets,
        Some(&vitals),
        None,
        Some(&ResourceBudgetInputs {
            transfer_bytes: Some(250_000),
            decoded_bytes: Some(900_000),
            lcp_media_bytes: Some(80_000),
        }),
    );
    assert!(!over.passed);
    assert!(
        over.violations
            .iter()
            .any(|v| v.metric == "transfer_bytes" && (v.actual - 250_000.0).abs() < 1.0),
        "violations={:?}",
        over.violations
    );

    let orch = FeatureOrchestrator::invoke(
        "perf-budgets",
        &json!({
            "budgets": {
                "max_transfer_bytes": 100000,
                "max_decoded_inflation_ratio": 2.0,
                "max_lcp_media_bytes": 50000
            },
            "vitals": { "lcp_ms": 1000.0 },
            "resources": {
                "transfer_bytes": 250000,
                "decoded_bytes": 900000,
                "lcp_media_bytes": 80000
            }
        }),
    )
    .expect("perf-budgets");
    assert_eq!(orch["result"]["passed"], false);
    let viols = orch["result"]["violations"].as_array().unwrap();
    assert!(viols.iter().any(|v| v["metric"] == "transfer_bytes"));
}

#[test]
fn test_wave4_analyzer_offline_naming_honesty() {
    for id in [
        FeatureId::DomXssTaintMap,
        FeatureId::RaceWindowRepeater,
        FeatureId::GrpcStreamWorkbench,
        FeatureId::MultipartWorkbench,
    ] {
        let name = id.name();
        let honest = name.contains("Analyzer") || name.to_lowercase().contains("offline");
        assert!(
            honest,
            "{} display name must contain Analyzer or offline honesty, got {name:?}",
            id.slug()
        );
    }
}
