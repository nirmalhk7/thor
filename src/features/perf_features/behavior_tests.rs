use super::*;

#[test]
fn cache_counts_present_empty_header_and_keeps_hit_then_age_issue_order() {
    let entries = [
        CacheEntry {
            url: "/one".into(),
            cache_control: Some("MAX-AGE=0, max-age=10".into()),
            age_secs: Some(1),
            x_cache: Some("hit".into()),
        },
        CacheEntry {
            url: "/two".into(),
            cache_control: Some("max-age=bad, max-age=1".into()),
            age_secs: Some(99),
            x_cache: Some("".into()),
        },
        CacheEntry {
            url: "/three".into(),
            cache_control: None,
            age_secs: None,
            x_cache: None,
        },
    ];
    let report = PerfEngine::new().analyze_cache(&entries);
    assert_eq!(report.hit_ratio, 0.5);
    assert_eq!(report.uncacheable, ["/one", "/three"]);
    assert_eq!(
        report.misconfigured,
        [
            "/one served from cache but Cache-Control is non-cacheable",
            "/one Age 1s exceeds max-age 0s"
        ]
    );
}

#[test]
fn lcp_invalid_nested_identity_blocks_flat_fallback_and_numeric_aliases_keep_precedence() {
    let payload = LcpElementPayload {
        vitals_json: Some(r#"{"lcpElement":{"url":null,"type":9,"size":"bad","discoverTime":10,"loadTime":20,"renderTime":30},"lcpUrl":"flat","lcpType":"image","lcpSize":100,"lcpDiscover":3,"lcpLoad":"bad","lcp":null,"lcp_ms":50}"#.into()),
        ..Default::default()
    };
    let report = PerfEngine::new().lcp_element_forensics(&payload);
    assert_eq!(report.url, "");
    assert_eq!(report.element_type, "unknown");
    assert_eq!(report.size_bytes, 0);
    assert_eq!(report.phases.discover_ms, 3.0);
    assert_eq!(report.phases.load_ms, 17.0);
    assert_eq!(report.phases.render_ms, 30.0);
    assert_eq!(report.phases.total_ms, 50.0);
}

#[test]
fn lcp_explicit_negative_timing_falls_back_then_clamps_phase_deltas() {
    let payload = LcpElementPayload {
        url: Some("explicit".into()),
        element_type: Some("text".into()),
        size_bytes: Some(7),
        discover_ms: Some(-1.0),
        load_ms: Some(5.0),
        render_ms: Some(2.0),
        vitals_json: Some(
            r#"{"lcpDiscover":10,"lcpLoad":100,"lcp":9000,"lcpUrl":"fallback"}"#.into(),
        ),
    };
    let report = PerfEngine::new().lcp_element_forensics(&payload);
    assert_eq!(
        (
            report.url.as_str(),
            report.element_type.as_str(),
            report.size_bytes
        ),
        ("explicit", "text", 7)
    );
    assert_eq!(report.phases.discover_ms, 10.0);
    assert_eq!(report.phases.load_ms, 0.0);
    assert_eq!(report.phases.render_ms, 0.0);
    assert_eq!(report.phases.total_ms, 10.0);
    assert_eq!(report.rating, "good");
}

#[test]
fn compression_keeps_thresholds_and_two_flags_for_missing_encoding_in_order() {
    let entries = [
        CompressionEntry {
            url: "/big".into(),
            transfer_bytes: 1025,
            decoded_bytes: 10000,
            content_encoding: None,
            content_type: Some("APPLICATION/JSON".into()),
        },
        CompressionEntry {
            url: "/edge".into(),
            transfer_bytes: 1024,
            decoded_bytes: 1000,
            content_encoding: None,
            content_type: Some("text/plain".into()),
        },
        CompressionEntry {
            url: "/gzip-edge".into(),
            transfer_bytes: 2000,
            decoded_bytes: 50000,
            content_encoding: Some("GZIP".into()),
            content_type: Some("text/plain".into()),
        },
        CompressionEntry {
            url: "/gzip-big".into(),
            transfer_bytes: 2000,
            decoded_bytes: 50001,
            content_encoding: Some("gzip".into()),
            content_type: Some("text/plain".into()),
        },
    ];
    let report = PerfEngine::new().compression_budget(&entries, 2.0);
    let flags: Vec<_> = report
        .weak_encoding
        .iter()
        .map(|f| (f.url.as_str(), f.reason.as_str()))
        .collect();
    assert_eq!(
        flags,
        [
            ("/big", "missing content-encoding on compressible asset"),
            ("/big", "decoded/transfer ratio 9.8 without encoding"),
            ("/gzip-big", "gzip on large payload; prefer br/zstd")
        ]
    );
    assert_eq!(report.total_transfer_bytes, 6049);
    assert_eq!(report.total_decoded_bytes, 111001);
    assert!(!report.passed);
}
