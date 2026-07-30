//! Wave-5 P0 regression pack for the Thor Engineer Feature Suite.
//!
//! Covers CI-bundle non-empty artifacts, SoftNav / LCP media attachment on
//! load metrics, and quarantine persist success + hydrate.

use serde_json::json;

use thor::dx::test_generator::RecordedSession;
use thor::features::cdp_bridge::CdpBridge;
use thor::features::orchestrator::FeatureOrchestrator;
use thor::features::perf_features::PerfEngine;
use thor::features::qa::{self, QuarantineInput};
use thor::features::{FeatureEngine, FeatureId};
use thor::load::{LoadMetrics, SoftNavRecord, SoftNavVitalsReport};

#[test]
fn test_wave5_registry_still_144() {
    assert_eq!(FeatureId::ALL.len(), 144);
    assert_eq!(FeatureEngine::new().registry().len(), 144);
}

#[test]
fn test_orch_ci_bundle_artifacts_non_empty() {
    let session = RecordedSession::new("Wave5 Checkout", "https://shop.example/checkout");
    let orch = FeatureOrchestrator::invoke(
        "ci-bundle",
        &json!({ "session": session }),
    )
    .expect("ci-bundle orch");

    let result = orch.get("result").unwrap_or(&orch);
    let playwright = result["playwright_ts"].as_str().unwrap_or("");
    let hermetic = result["hermetic_ts"].as_str().unwrap_or("");
    let filter = result["quarantine_filter"].as_str().unwrap_or("");

    assert!(
        !playwright.trim().is_empty(),
        "playwright_ts must be non-empty"
    );
    assert!(!hermetic.trim().is_empty(), "hermetic_ts must be non-empty");
    assert!(
        !filter.trim().is_empty(),
        "quarantine_filter must be non-empty (stub allowed)"
    );

    // Direct engine path agrees.
    let m = qa::QaEngine::new().export_ci_bundle_composed(&session, None);
    assert!(!m.playwright_ts.trim().is_empty());
    assert!(!m.hermetic_ts.trim().is_empty());
    assert!(!m.quarantine_filter.trim().is_empty());
}

#[test]
fn test_soft_nav_report_and_lcp_media_from_mocked_vitals() {
    // lcpSize on mocked vitals → Some(lcp_media_bytes).
    let json = r#"{
        "lcp": 1800.0,
        "inp": 40.0,
        "cls": 0.02,
        "fcp": 900.0,
        "ttfb": 200.0,
        "lcpSize": 123456,
        "lcpElement": { "url": "https://cdn/hero.jpg", "type": "image", "size": 123456 }
    }"#;
    let (vitals, lcp_media) = CdpBridge::parse_web_vitals_ex(json).expect("parse vitals");
    assert!((vitals.lcp_ms - 1800.0).abs() < 1e-6);
    assert_eq!(lcp_media, Some(123_456));

    // Soft-nav Ok report attaches to LoadMetrics (refuse all-zero still holds).
    let perf = PerfEngine::new();
    let report = perf
        .soft_nav_vitals(&[SoftNavRecord {
            navigation_id: "n1".into(),
            url: "/app/dashboard".into(),
            lcp_ms: 420.0,
            inp_ms: 18.0,
            cls: 0.01,
            ttfb_ms: 12.0,
        }])
        .expect("non-zero soft-nav");
    assert_eq!(report.navigations, 1);

    let metrics = LoadMetrics {
        total_requests: 1,
        successful_requests: 1,
        failed_requests: 0,
        total_bytes: 0,
        duration_secs: 0.1,
        rps: 10.0,
        min_latency_ms: 10.0,
        max_latency_ms: 10.0,
        avg_latency_ms: 10.0,
        p50_latency_ms: 10.0,
        p90_latency_ms: 10.0,
        p95_latency_ms: 10.0,
        p99_latency_ms: 10.0,
        status_codes: Default::default(),
        browser_vitals: Some(vitals),
        transfer_bytes: None,
        decoded_bytes: None,
        lcp_media_bytes: lcp_media,
        soft_nav: Some(report.clone()),
    };
    assert!(metrics.lcp_media_bytes.is_some());
    let soft: &SoftNavVitalsReport = metrics.soft_nav.as_ref().expect("soft_nav attached");
    assert_eq!(soft.navigations, 1);
    assert!((soft.avg_lcp_ms - 420.0).abs() < 1e-6);

    // lcpElement.size fallback when lcpSize absent.
    let via_el = CdpBridge::parse_web_vitals_ex(
        r#"{"lcp":1000,"lcpElement":{"size":9999}}"#,
    )
    .expect("el size");
    assert_eq!(via_el.1, Some(9999));

    // Zero / missing size stays None.
    let none = CdpBridge::parse_web_vitals_ex(r#"{"lcp":1000,"lcpSize":0}"#).expect("zero");
    assert_eq!(none.1, None);

    // All-zero soft-nav still refused (not attached as theater).
    let refuse = perf.soft_nav_vitals(&[SoftNavRecord {
        navigation_id: "z".into(),
        url: "/z".into(),
        lcp_ms: 0.0,
        inp_ms: 0.0,
        cls: 0.0,
        ttfb_ms: 0.0,
    }]);
    assert!(refuse.is_err());
}

#[test]
fn test_quarantine_persist_surfaces_success_and_hydrate() {
    let tmp = std::env::temp_dir().join(format!(
        "thor-wave5-q-{}-{}",
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
        test: "wave5.spec.ts".into(),
        flake_rate: 0.8,
        consecutive_fails: 6,
        days_in_quarantine: 0.0,
        green_streak: 0,
        currently_quarantined: false,
    });
    assert_eq!(d.action, "quarantine");
    assert!(d.persist_ok, "persist must surface success");
    assert!(d.persist_err.is_none());
    assert!(qa::QaEngine::quarantine_path().exists());

    let orch = FeatureOrchestrator::invoke(
        "quarantine-policy",
        &json!({
            "input": {
                "test": "wave5-orch.spec.ts",
                "flake_rate": 0.9,
                "consecutive_fails": 4,
                "days_in_quarantine": 0.0,
                "green_streak": 0,
                "currently_quarantined": false
            }
        }),
    )
    .expect("quarantine-policy orch");
    let body = orch.get("result").unwrap_or(&orch);
    // persist_ok may sit at the envelope root and/or under `decision`
    // (orchestrator flattens the decision then nests a copy).
    let persist_ok = body
        .get("persist_ok")
        .or_else(|| body.pointer("/decision/persist_ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        persist_ok,
        "quarantine-policy must surface persist_ok=true, got: {body}"
    );
    let persist_err = body
        .get("persist_err")
        .or_else(|| body.pointer("/decision/persist_err"));
    assert!(
        persist_err.is_none()
            || persist_err.map(|v| v.is_null()).unwrap_or(false)
            || persist_err
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(false),
        "persist_err should be empty on success, got: {body}"
    );

    // Fresh engine hydrates from disk.
    let reader = qa::QaEngine::new();
    let status = reader
        .quarantine_status("wave5.spec.ts")
        .expect("hydrated wave5.spec.ts");
    assert_eq!(status.action, "quarantine");
    let orch_status = reader
        .quarantine_status("wave5-orch.spec.ts")
        .expect("hydrated orch entry");
    assert_eq!(orch_status.action, "quarantine");
    assert!(reader.ci_quarantine_filter().contains("wave5"));

    std::env::remove_var("THOR_DATA_DIR");
    let _ = std::fs::remove_dir_all(&tmp);
}
