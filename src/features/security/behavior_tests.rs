use super::*;

#[test]
fn header_score_applies_content_penalties_after_required_and_leak_findings() {
    let headers = BTreeMap::from([
        (
            "Content-Security-Policy".into(),
            "script-src 'UNSAFE-INLINE' 'unsafe-eval' *".into(),
        ),
        ("Strict-Transport-Security".into(), "".into()),
        ("X-Content-Type-Options".into(), "".into()),
        ("X-Frame-Options".into(), "".into()),
        ("Referrer-Policy".into(), "".into()),
        ("Permissions-Policy".into(), "".into()),
        ("Server".into(), "test".into()),
        ("X-Powered-By".into(), "test".into()),
        ("Access-Control-Allow-Origin".into(), " * ".into()),
        ("Access-Control-Allow-Credentials".into(), "TRUE".into()),
    ]);
    let report = SecurityEngine::new().analyze_headers(&headers);
    assert_eq!(report.score, 45);
    assert_eq!(report.grade, "D");
    let names: Vec<_> = report.findings.iter().map(|f| f.header.as_str()).collect();
    assert_eq!(
        names,
        [
            "content-security-policy",
            "strict-transport-security",
            "x-content-type-options",
            "x-frame-options",
            "referrer-policy",
            "permissions-policy",
            "server",
            "x-powered-by",
            "content-security-policy",
            "content-security-policy",
            "content-security-policy",
            "access-control-allow-origin"
        ]
    );
    assert!(report.findings[..6]
        .iter()
        .all(|f| f.present && f.recommendation == "OK"));
    assert_eq!(report.findings[8].severity, "medium");
    assert_eq!(report.findings[10].severity, "low");
    assert_eq!(report.findings[11].severity, "high");
}

#[test]
fn deep_cors_preserves_exact_credentials_case_and_star_precedence() {
    let report = SecurityEngine::new().cors_deep_probe(&[
        CorsProbeResult {
            origin: "*".into(),
            acao: Some("*".into()),
            acac: Some("TRUE".into()),
            method: "GET".into(),
        },
        CorsProbeResult {
            origin: "null".into(),
            acao: Some("null".into()),
            acac: Some("true".into()),
            method: "POST".into(),
        },
        CorsProbeResult {
            origin: "https://test".into(),
            acao: Some(" https://test".into()),
            acac: Some("true".into()),
            method: "GET".into(),
        },
    ]);
    assert_eq!(
        report.findings,
        [
            "ACAO * for origin *",
            "reflects Origin: null",
            "reflected origin null with Access-Control-Allow-Credentials: true"
        ]
    );
    assert!(report.reflects_origin);
    assert!(report.credentials_with_reflected_origin);
    assert!(!report.credentials_with_star);
}

#[test]
fn cookies_count_session_names_or_missing_age_and_report_byte_size_last() {
    let cookies = vec![
        CookieJarEntry {
            name: "SIDeline".into(),
            value: "雪".repeat(1366),
            secure: false,
            http_only: false,
            same_site: Some("NoNe".into()),
            domain: None,
            path: None,
            max_age: Some(60),
        },
        CookieJarEntry {
            name: "other".into(),
            value: "".into(),
            secure: true,
            http_only: true,
            same_site: Some("LAX".into()),
            domain: None,
            path: None,
            max_age: None,
        },
        CookieJarEntry {
            name: "persist".into(),
            value: "x".repeat(4097),
            secure: false,
            http_only: false,
            same_site: None,
            domain: None,
            path: None,
            max_age: Some(60),
        },
    ];
    let report = SecurityEngine::new().cookie_session_forensics(&cookies);
    assert_eq!(report.session_cookies, 2);
    assert_eq!(
        report.issues,
        [
            "SIDeline missing Secure",
            "SIDeline missing HttpOnly",
            "SIDeline SameSite=None without Secure",
            "SIDeline value exceeds 4KB",
            "persist value exceeds 4KB"
        ]
    );
}

#[test]
fn ssrf_preflight_refuses_first_public_target_and_never_allows_metadata() {
    let engine = SecurityEngine::new();
    let targets = vec![
        SsrfTarget {
            url: "http://169.254.169.254/latest/meta-data/".into(),
            resolved_ip: Some("127.0.0.1".into()),
        },
        SsrfTarget {
            url: "http://127.0.0.1/local".into(),
            resolved_ip: Some("127.2.3.4".into()),
        },
        SsrfTarget {
            url: "https://example.com/a".into(),
            resolved_ip: Some("0.0.0.0".into()),
        },
        SsrfTarget {
            url: "https://example.org/b".into(),
            resolved_ip: None,
        },
    ];
    assert_eq!(
        engine.ssrf_lab_harness(&targets, false).unwrap_err(),
        "SSRF lab refused: public target without lab_consent: https://example.com/a"
    );
    let report = engine.ssrf_lab_harness(&targets, true).unwrap();
    assert_eq!(report.blocked, ["http://169.254.169.254/latest/meta-data/"]);
    assert_eq!(
        report.allowed_lab,
        [
            "http://127.0.0.1/local",
            "https://example.com/a",
            "https://example.org/b"
        ]
    );
    assert_eq!(
        report.issues,
        [
            "blocked abuse/metadata target http://169.254.169.254/latest/meta-data/",
            "http://127.0.0.1/local resolved to sensitive IP 127.2.3.4",
            "lab-consented public target https://example.com/a",
            "https://example.com/a resolved to sensitive IP 0.0.0.0",
            "lab-consented public target https://example.org/b"
        ]
    );
}

#[test]
fn taint_classification_keeps_sink_priority_and_explicit_empty_kind() {
    let flows = SecurityEngine::parse_dom_xss_taint_flows(
        r#"[
        {"source":"x","sink":"innerHTML eval location"},
        {"source":"x","sink":"location SCRIPT"},
        {"source":"x","sink":"SetAttribute src"},
        {"source":"x","sink":"eval","sink_kind":""}
    ]"#,
    );
    let report = SecurityEngine::new().dom_xss_taint_map(&flows);
    let kinds: Vec<_> = report
        .flows
        .iter()
        .map(|f| f.sink_kind.as_deref())
        .collect();
    assert_eq!(kinds, [Some("html"), Some("js"), Some("url"), Some("")]);
    assert_eq!(report.unsanitized, 4);
    assert_eq!(report.risk, "high");
}
