//! Security specialist features (12 + 12 wave-2).
//!
//! Authorization matrix, response diff, security-header analysis, deep JWT
//! inspection, promote-to-finding, attack-surface crawling, storage auditing,
//! fuzz repeater, CSRF analysis, sink/pollution detection, TLS inspection, and
//! an out-of-band interaction listener. Reuses [`crate::dx::decoder`] and
//! [`crate::dx::test_generator`] where appropriate.

mod authorization;
mod cookies;
mod cors;
mod crypto;
mod csp;
mod csrf;
mod findings;
mod fuzz;
mod headers;
mod jwt;
mod oidc;
mod oob;
mod races;
mod response;
mod sinks;
mod sri;
mod ssrf;
mod storage;
mod surface;
mod tls;
mod websocket;
mod xss;

pub(crate) use crypto::{hex_encode, hmac_sha256};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::app_state::AppState;
use crate::dx::decoder::DecoderStudio;
use crate::dx::test_generator::{NetworkTrace, RecordedSession, TestGenerator};

// ---------------------------------------------------------------------------
// 1. Authorization Matrix
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzProbe {
    pub role: String,
    pub endpoint: String,
    /// Whether the role is *expected* to have access.
    pub should_allow: bool,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzCell {
    pub role: String,
    pub endpoint: String,
    pub allowed: bool,
    pub expected: bool,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzMatrixReport {
    pub cells: Vec<AuthzCell>,
    pub escalations: usize,
    pub over_restrictions: usize,
}

// ---------------------------------------------------------------------------
// 2. Response Diff
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpSnapshot {
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseDiffReport {
    pub status_changed: Option<(u16, u16)>,
    pub header_changes: Vec<String>,
    pub added_lines: Vec<String>,
    pub removed_lines: Vec<String>,
    pub similarity: f64,
}

// ---------------------------------------------------------------------------
// 3. Header Analyzer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderFinding {
    pub header: String,
    pub present: bool,
    pub severity: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderReport {
    pub findings: Vec<HeaderFinding>,
    pub grade: String,
    pub score: u32,
}

// ---------------------------------------------------------------------------
// 4. JWT Deep
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtDeepReport {
    pub algorithm: String,
    pub header_json: Option<String>,
    pub payload_json: Option<String>,
    pub issues: Vec<String>,
    pub expired: Option<bool>,
}

// ---------------------------------------------------------------------------
// 6. Surface Crawler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceEndpoint {
    pub method: String,
    pub path_template: String,
    pub params: Vec<String>,
    pub instances: usize,
}

// ---------------------------------------------------------------------------
// 7. Storage Auditor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageItem {
    pub store: String, // "cookie" | "localStorage" | "sessionStorage"
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub same_site: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageFinding {
    pub key: String,
    pub store: String,
    pub severity: String,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// 8. Fuzz Repeater
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuzzCategory {
    Sqli,
    Xss,
    PathTraversal,
    CommandInjection,
    Ssti,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzCase {
    pub category: String,
    pub payload: String,
    pub mutated: String,
}

// ---------------------------------------------------------------------------
// 9. CSRF Analyzer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsrfContext {
    pub method: String,
    #[serde(default)]
    pub has_csrf_token: bool,
    #[serde(default)]
    pub cookie_same_site: Option<String>,
    #[serde(default)]
    pub uses_cookie_auth: bool,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsrfReport {
    pub vulnerable: bool,
    pub issues: Vec<String>,
}

// ---------------------------------------------------------------------------
// 10. Sink / Pollution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkFinding {
    pub kind: String,
    pub location: String,
    pub severity: String,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// 11. TLS Inspector
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsInfo {
    pub protocol: String, // e.g. "TLS1.3"
    pub cipher: String,
    pub cert_days_remaining: i64,
    #[serde(default)]
    pub self_signed: bool,
    #[serde(default)]
    pub hostname_matches: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsReport {
    pub grade: String,
    pub issues: Vec<String>,
}

// ---------------------------------------------------------------------------
// 12. OOB Listener
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OobInteraction {
    pub token: String,
    pub protocol: String, // "dns" | "http"
    pub source_ip: String,
    pub received_ms: u64,
    pub detail: String,
}

/// Live localhost OOB listener handle.
type OobSinkHandle = super::loopback::LoopbackHandle;

/// Security specialist engine.
pub struct SecurityEngine {
    oob_domain: String,
    oob_counter: Mutex<u64>,
    oob_interactions: Mutex<Vec<OobInteraction>>,
    oob_sink: Mutex<Option<OobSinkHandle>>,
}

impl Default for SecurityEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Wave-2 security features (12)
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaintFlow {
    pub source: String,
    pub sink: String,
    #[serde(default)]
    pub via: Vec<String>,
    #[serde(default)]
    pub sanitized: bool,
    /// Sink taxonomy: `html` | `js` | `url` | `dom` | `unknown`.
    #[serde(default)]
    pub sink_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomXssTaintReport {
    pub flows: Vec<TaintFlow>,
    pub unsanitized: usize,
    pub risk: String,
    /// Counts by sink taxonomy for CDP-shaped triage.
    #[serde(default)]
    pub by_sink_kind: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsProbeResult {
    pub origin: String,
    pub acao: Option<String>,
    pub acac: Option<String>,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsDeepReport {
    pub findings: Vec<String>,
    pub reflects_origin: bool,
    pub credentials_with_star: bool,
    /// True when ACAO reflects the request Origin and ACAC is true.
    #[serde(default)]
    pub credentials_with_reflected_origin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CspForensicsReport {
    pub directives: BTreeMap<String, Vec<String>>,
    pub issues: Vec<String>,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: Option<String>,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    #[serde(default)]
    pub claims: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcMisconfigReport {
    pub issues: Vec<String>,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieJarEntry {
    pub name: String,
    pub value: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub max_age: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieForensicsReport {
    pub session_cookies: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFixationProbe {
    pub pre_auth_session: String,
    pub post_auth_session: String,
    pub login_status: u16,
    /// Login URL under test — required for consent gating (always).
    pub target_url: String,
    /// Explicit authorized-lab consent for non-local targets.
    #[serde(default)]
    pub lab_consent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFixationReport {
    pub vulnerable: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphqlAuthzProbe {
    pub role: String,
    pub operation: String,
    pub field: String,
    pub should_allow: bool,
    pub status: u16,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphqlAuthzReport {
    pub escalations: usize,
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassAssignDiff {
    pub sent_fields: Vec<String>,
    pub accepted_fields: Vec<String>,
    pub privileged_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassAssignReport {
    pub unexpected_writes: Vec<String>,
    pub privileged_accepted: Vec<String>,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceAttempt {
    pub id: u32,
    pub status: u16,
    pub body_hash: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceWindowReport {
    pub attempts: usize,
    pub unique_outcomes: usize,
    pub window_ms: u64,
    pub inconsistency: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsrfTarget {
    pub url: String,
    pub resolved_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsrfLabReport {
    pub blocked: Vec<String>,
    pub allowed_lab: Vec<String>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SriScript {
    pub src: String,
    pub integrity: Option<String>,
    pub crossorigin: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SriAuditReport {
    pub missing_integrity: Vec<String>,
    pub weak_algo: Vec<String>,
    pub missing_crossorigin: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsAuthzProbe {
    pub role: String,
    pub url: String,
    pub connected: bool,
    pub should_allow: bool,
    pub first_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsAuthzReport {
    pub escalations: usize,
    pub findings: Vec<String>,
}

impl SecurityEngine {
    pub fn new() -> Self {
        Self {
            oob_domain: "oob.thor.local".to_string(),
            oob_counter: Mutex::new(0),
            oob_interactions: Mutex::new(Vec::new()),
            oob_sink: Mutex::new(None),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_authz_matrix_detects_escalation() {
        let e = SecurityEngine::new();
        let probes = vec![
            AuthzProbe {
                role: "user".into(),
                endpoint: "/admin".into(),
                should_allow: false,
                status: 200,
            },
            AuthzProbe {
                role: "admin".into(),
                endpoint: "/admin".into(),
                should_allow: true,
                status: 200,
            },
        ];
        let r = e.build_authz_matrix(&probes);
        assert_eq!(r.escalations, 1);
        assert!(r.cells.iter().any(|c| c.verdict == "PRIVILEGE-ESCALATION"));
    }

    #[test]
    fn test_response_diff() {
        let e = SecurityEngine::new();
        let a = HttpSnapshot {
            status: 200,
            headers: BTreeMap::new(),
            body: "line1\nline2".into(),
        };
        let mut hb = BTreeMap::new();
        hb.insert("X-New".to_string(), "1".to_string());
        let b = HttpSnapshot {
            status: 403,
            headers: hb,
            body: "line1\nline3".into(),
        };
        let d = e.diff_responses(&a, &b);
        assert_eq!(d.status_changed, Some((200, 403)));
        assert!(d.added_lines.contains(&"line3".to_string()));
        assert!(d.removed_lines.contains(&"line2".to_string()));
        assert!(d.header_changes.iter().any(|c| c.contains("X-New")));
    }

    #[test]
    fn test_header_analyzer_grades() {
        let e = SecurityEngine::new();
        let empty = e.analyze_headers(&BTreeMap::new());
        assert_eq!(empty.grade, "F");
        let mut full = BTreeMap::new();
        for (h, v) in [
            ("content-security-policy", "default-src 'self'"),
            ("strict-transport-security", "max-age=63072000"),
            ("x-content-type-options", "nosniff"),
            ("x-frame-options", "DENY"),
            ("referrer-policy", "no-referrer"),
            ("permissions-policy", "geolocation=()"),
        ] {
            full.insert(h.to_string(), v.to_string());
        }
        let r = e.analyze_headers(&full);
        assert_eq!(r.grade, "A");
        assert_eq!(r.score, 100);
    }

    #[test]
    fn test_jwt_deep_none_alg() {
        let e = SecurityEngine::new();
        // header {"alg":"none"} . payload {"sub":"1"} . (empty sig placeholder)
        let jwt = "eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0.x";
        let r = e.inspect_jwt(jwt, 0).unwrap();
        assert_eq!(r.algorithm.to_lowercase(), "none");
        assert!(r.issues.iter().any(|i| i.contains("alg=none")));
        assert!(r.issues.iter().any(|i| i.contains("missing 'exp'")));
    }

    #[test]
    fn test_surface_crawler_templatizes() {
        let e = SecurityEngine::new();
        let discovered = vec![
            ("GET".into(), "/api/users/1".into()),
            ("GET".into(), "/api/users/2?fields=name".into()),
        ];
        let s = e.crawl_surface(&discovered);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].path_template, "/api/users/:id");
        assert_eq!(s[0].instances, 2);
        assert!(s[0].params.contains(&"fields".to_string()));
    }

    #[test]
    fn test_storage_auditor() {
        let e = SecurityEngine::new();
        let items = vec![
            StorageItem {
                store: "localStorage".into(),
                key: "auth_token".into(),
                value: "x".into(),
                http_only: false,
                secure: false,
                same_site: None,
            },
            StorageItem {
                store: "cookie".into(),
                key: "session".into(),
                value: "y".into(),
                http_only: false,
                secure: false,
                same_site: None,
            },
        ];
        let f = e.audit_storage(&items);
        assert!(f
            .iter()
            .any(|x| x.store == "localStorage" && x.severity == "high"));
        assert!(f.iter().any(|x| x.detail.contains("HttpOnly")));
    }

    #[test]
    fn test_fuzz_cases() {
        let e = SecurityEngine::new();
        let cases = e.build_fuzz_cases("/search?q=FUZZ", FuzzCategory::Xss);
        assert!(!cases.is_empty());
        assert!(cases.iter().all(|c| c.mutated.starts_with("/search?q=")));
        assert!(cases.iter().any(|c| c.mutated.contains("<script>")));
    }

    #[test]
    fn test_csrf_analyzer() {
        let e = SecurityEngine::new();
        let vuln = CsrfContext {
            method: "POST".into(),
            has_csrf_token: false,
            cookie_same_site: None,
            uses_cookie_auth: true,
            content_type: Some("application/x-www-form-urlencoded".into()),
        };
        assert!(e.analyze_csrf(&vuln).vulnerable);
        let safe = CsrfContext {
            method: "POST".into(),
            has_csrf_token: true,
            cookie_same_site: Some("Strict".into()),
            uses_cookie_auth: true,
            content_type: Some("application/json".into()),
        };
        assert!(!e.analyze_csrf(&safe).vulnerable);
        let bearer = CsrfContext {
            method: "POST".into(),
            has_csrf_token: false,
            cookie_same_site: None,
            uses_cookie_auth: false,
            content_type: None,
        };
        assert!(!e.analyze_csrf(&bearer).vulnerable);
    }

    #[test]
    fn test_sink_pollution() {
        let e = SecurityEngine::new();
        let findings = e.detect_sinks(
            &json!({"__proto__": {"isAdmin": true}}),
            Some("el.innerHTML = userInput; eval(x)"),
        );
        assert!(findings.iter().any(|f| f.kind == "prototype-pollution"));
        assert!(findings
            .iter()
            .any(|f| f.location == "eval(" && f.severity == "critical"));
    }

    #[test]
    fn test_tls_inspector() {
        let e = SecurityEngine::new();
        let good = TlsInfo {
            protocol: "TLS1.3".into(),
            cipher: "TLS_AES_256_GCM_SHA384".into(),
            cert_days_remaining: 90,
            self_signed: false,
            hostname_matches: true,
        };
        assert_eq!(e.inspect_tls(&good).grade, "A+");
        let bad = TlsInfo {
            protocol: "TLS1.0".into(),
            cipher: "RC4-MD5".into(),
            cert_days_remaining: -1,
            self_signed: true,
            hostname_matches: false,
        };
        let r = e.inspect_tls(&bad);
        assert_eq!(r.grade, "F");
        assert!(r.issues.len() >= 4);
    }

    #[test]
    fn test_oob_listener() {
        let e = SecurityEngine::new();
        let t1 = e.new_oob_token();
        let t2 = e.new_oob_token();
        assert_ne!(t1, t2);
        e.record_oob(OobInteraction {
            token: t1.clone(),
            protocol: "dns".into(),
            source_ip: "1.2.3.4".into(),
            received_ms: 1,
            detail: "A query".into(),
        });
        assert_eq!(e.oob_interactions(&t1).len(), 1);
        assert_eq!(e.oob_interactions(&t2).len(), 0);
    }

    #[test]
    fn test_promote_finding_uses_generator() {
        let e = SecurityEngine::new();
        let traces = vec![NetworkTrace {
            method: "GET".into(),
            url: "https://x/y".into(),
            headers: HashMap::new(),
            body: None,
            status: 500,
        }];
        let md = e.promote_to_finding("Broken endpoint", "https://x", traces);
        assert!(md.contains("# Finding: Broken endpoint"));
    }

    #[test]
    fn test_csp_and_cors_wave2() {
        let e = SecurityEngine::new();
        let csp = e.csp_policy_forensics("default-src 'self'; script-src 'unsafe-inline'");
        assert!(csp.score < 100);
        assert!(!csp.issues.is_empty());
        let cors = e.cors_deep_probe(&[CorsProbeResult {
            origin: "https://evil.test".into(),
            acao: Some("*".into()),
            acac: Some("true".into()),
            method: "GET".into(),
        }]);
        assert!(cors.credentials_with_star);
        let reflect = e.cors_deep_probe(&[CorsProbeResult {
            origin: "https://evil.test".into(),
            acao: Some("https://evil.test".into()),
            acac: Some("true".into()),
            method: "GET".into(),
        }]);
        assert!(reflect.reflects_origin);
        assert!(reflect.credentials_with_reflected_origin);
    }

    #[test]
    fn test_session_fixation_and_mass_assign() {
        let e = SecurityEngine::new();
        assert!(e
            .session_fixation_lab(&SessionFixationProbe {
                pre_auth_session: "abc".into(),
                post_auth_session: "abc".into(),
                login_status: 200,
                target_url: "".into(),
                lab_consent: false,
            })
            .is_err());
        let fix = e
            .session_fixation_lab(&SessionFixationProbe {
                pre_auth_session: "abc".into(),
                post_auth_session: "abc".into(),
                login_status: 200,
                target_url: "http://127.0.0.1/login".into(),
                lab_consent: false,
            })
            .unwrap();
        assert!(fix.vulnerable);
        assert!(e
            .session_fixation_lab(&SessionFixationProbe {
                pre_auth_session: "a".into(),
                post_auth_session: "b".into(),
                login_status: 200,
                target_url: "https://example.com/login".into(),
                lab_consent: false,
            })
            .is_err());
        let ma = e.mass_assignment_diff(&MassAssignDiff {
            sent_fields: vec!["name".into()],
            accepted_fields: vec!["name".into(), "role".into()],
            privileged_fields: vec!["role".into()],
        });
        assert_eq!(ma.risk, "high");
    }

    #[test]
    fn test_ssrf_consent_and_dom_xss_taint() {
        let e = SecurityEngine::new();
        assert!(e
            .ssrf_lab_harness(
                &[SsrfTarget {
                    url: "https://example.com/".into(),
                    resolved_ip: None,
                }],
                false,
            )
            .is_err());
        let ok = e
            .ssrf_lab_harness(
                &[SsrfTarget {
                    url: "http://127.0.0.1/admin".into(),
                    resolved_ip: Some("127.0.0.1".into()),
                }],
                false,
            )
            .unwrap();
        assert!(!ok.allowed_lab.is_empty());
        let flows = SecurityEngine::parse_dom_xss_taint_flows(
            r#"[{"source":"location.hash","sink":"el.innerHTML","via":["assign"],"sanitized":false}]"#,
        );
        let t = e.dom_xss_taint_map(&flows);
        assert_eq!(t.unsanitized, 1);
        assert_eq!(t.by_sink_kind.get("html"), Some(&1));
    }

    #[test]
    fn test_sri_and_graphql_authz() {
        let e = SecurityEngine::new();
        let sri = e.sri_supply_chain_auditor(&[SriScript {
            src: "https://cdn/x.js".into(),
            integrity: None,
            crossorigin: None,
            bytes: Some(10),
        }]);
        assert_eq!(sri.missing_integrity.len(), 1);
        let g = e.graphql_authz_matrix(&[GraphqlAuthzProbe {
            role: "user".into(),
            operation: "query".into(),
            field: "adminUsers".into(),
            should_allow: false,
            status: 200,
            errors: vec![],
        }]);
        assert_eq!(g.escalations, 1);
    }
}

#[cfg(test)]
mod behavior_tests;
