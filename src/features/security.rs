//! Security specialist features (12 + 12 wave-2).
//!
//! Authorization matrix, response diff, security-header analysis, deep JWT
//! inspection, promote-to-finding, attack-surface crawling, storage auditing,
//! fuzz repeater, CSRF analysis, sink/pollution detection, TLS inspection, and
//! an out-of-band interaction listener. Reuses [`crate::dx::decoder`] and
//! [`crate::dx::test_generator`] where appropriate.

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
struct OobSinkHandle {
    port: u16,
    running: Arc<AtomicBool>,
}

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

    // 1. Authorization Matrix ----------------------------------------------
    pub fn build_authz_matrix(&self, probes: &[AuthzProbe]) -> AuthzMatrixReport {
        let mut cells = Vec::new();
        let mut escalations = 0;
        let mut over_restrictions = 0;
        for p in probes {
            let allowed = (200..300).contains(&p.status);
            let verdict = match (allowed, p.should_allow) {
                (true, false) => {
                    escalations += 1;
                    "PRIVILEGE-ESCALATION"
                }
                (false, true) => {
                    over_restrictions += 1;
                    "OVER-RESTRICTED"
                }
                _ => "OK",
            };
            cells.push(AuthzCell {
                role: p.role.clone(),
                endpoint: p.endpoint.clone(),
                allowed,
                expected: p.should_allow,
                verdict: verdict.to_string(),
            });
        }
        AuthzMatrixReport {
            cells,
            escalations,
            over_restrictions,
        }
    }

    // 2. Response Diff ------------------------------------------------------
    pub fn diff_responses(&self, a: &HttpSnapshot, b: &HttpSnapshot) -> ResponseDiffReport {
        let status_changed = if a.status != b.status {
            Some((a.status, b.status))
        } else {
            None
        };

        let mut header_changes = Vec::new();
        let mut keys: Vec<&String> = a.headers.keys().chain(b.headers.keys()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            match (a.headers.get(k), b.headers.get(k)) {
                (Some(va), Some(vb)) if va != vb => {
                    header_changes.push(format!("~ {k}: {va} -> {vb}"))
                }
                (Some(va), None) => header_changes.push(format!("- {k}: {va}")),
                (None, Some(vb)) => header_changes.push(format!("+ {k}: {vb}")),
                _ => {}
            }
        }

        let a_lines: Vec<&str> = a.body.lines().collect();
        let b_lines: Vec<&str> = b.body.lines().collect();
        let a_set: std::collections::HashSet<&str> = a_lines.iter().copied().collect();
        let b_set: std::collections::HashSet<&str> = b_lines.iter().copied().collect();
        let added_lines: Vec<String> = b_lines
            .iter()
            .filter(|l| !a_set.contains(*l))
            .map(|s| s.to_string())
            .collect();
        let removed_lines: Vec<String> = a_lines
            .iter()
            .filter(|l| !b_set.contains(*l))
            .map(|s| s.to_string())
            .collect();

        let common = a_set.intersection(&b_set).count();
        let union = a_set.union(&b_set).count().max(1);
        let similarity = common as f64 / union as f64;

        ResponseDiffReport {
            status_changed,
            header_changes,
            added_lines,
            removed_lines,
            similarity,
        }
    }

    // 3. Header Analyzer ----------------------------------------------------
    pub fn analyze_headers(&self, headers: &BTreeMap<String, String>) -> HeaderReport {
        let lower: BTreeMap<String, String> = headers
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect();
        // (header, severity_weight, recommendation)
        let required = [
            (
                "content-security-policy",
                30u32,
                "Add a restrictive CSP to mitigate XSS",
            ),
            (
                "strict-transport-security",
                20,
                "Add HSTS with a long max-age",
            ),
            (
                "x-content-type-options",
                10,
                "Set X-Content-Type-Options: nosniff",
            ),
            (
                "x-frame-options",
                15,
                "Set X-Frame-Options or CSP frame-ancestors",
            ),
            ("referrer-policy", 10, "Set a Referrer-Policy"),
            (
                "permissions-policy",
                5,
                "Restrict powerful features via Permissions-Policy",
            ),
        ];
        let mut findings = Vec::new();
        let mut score = 100i64;
        for (h, weight, rec) in required {
            let present = lower.contains_key(h);
            if !present {
                score -= weight as i64;
            }
            findings.push(HeaderFinding {
                header: h.to_string(),
                present,
                severity: if weight >= 20 {
                    "high".into()
                } else if weight >= 10 {
                    "medium".into()
                } else {
                    "low".into()
                },
                recommendation: if present {
                    "OK".into()
                } else {
                    rec.to_string()
                },
            });
        }
        // Info leaks reduce score.
        for leak in ["server", "x-powered-by"] {
            if lower.contains_key(leak) {
                score -= 5;
                findings.push(HeaderFinding {
                    header: leak.to_string(),
                    present: true,
                    severity: "low".into(),
                    recommendation: format!("Remove `{leak}` to avoid version disclosure"),
                });
            }
        }
        // CSP content weaknesses (only when a policy is actually present).
        if let Some(csp) = lower.get("content-security-policy") {
            let c = csp.to_lowercase();
            if c.contains("unsafe-inline") {
                score -= 10;
                findings.push(HeaderFinding {
                    header: "content-security-policy".into(),
                    present: true,
                    severity: "medium".into(),
                    recommendation: "CSP allows 'unsafe-inline' — weakens XSS mitigation".into(),
                });
            }
            if c.contains("unsafe-eval") {
                score -= 10;
                findings.push(HeaderFinding {
                    header: "content-security-policy".into(),
                    present: true,
                    severity: "medium".into(),
                    recommendation: "CSP allows 'unsafe-eval' — enables string-to-code".into(),
                });
            }
            let wildcard_source = c.split(';').any(|d| {
                let d = d.trim();
                d.ends_with(" *") || d.ends_with(":*") || d.split_whitespace().any(|t| t == "*")
            });
            if wildcard_source {
                score -= 5;
                findings.push(HeaderFinding {
                    header: "content-security-policy".into(),
                    present: true,
                    severity: "low".into(),
                    recommendation: "CSP uses a wildcard (*) source — tighten allowed origins".into(),
                });
            }
        }
        // CORS analysis: wildcard ACAO (with credentials) is a critical misconfig.
        if let Some(acao) = lower.get("access-control-allow-origin") {
            let creds = lower
                .get("access-control-allow-credentials")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if acao.trim() == "*" && creds {
                score -= 20;
                findings.push(HeaderFinding {
                    header: "access-control-allow-origin".into(),
                    present: true,
                    severity: "high".into(),
                    recommendation:
                        "CORS: wildcard ACAO with Allow-Credentials — reflects any origin".into(),
                });
            } else if acao.trim() == "*" {
                findings.push(HeaderFinding {
                    header: "access-control-allow-origin".into(),
                    present: true,
                    severity: "low".into(),
                    recommendation: "CORS: wildcard ACAO exposes the endpoint to any origin".into(),
                });
            }
        }
        let score = score.max(0) as u32;
        let grade = match score {
            90..=100 => "A",
            75..=89 => "B",
            60..=74 => "C",
            40..=59 => "D",
            _ => "F",
        };
        HeaderReport {
            findings,
            grade: grade.to_string(),
            score,
        }
    }

    // 4. JWT Deep -----------------------------------------------------------
    pub fn inspect_jwt(&self, jwt: &str, now_unix: u64) -> Result<JwtDeepReport, String> {
        let decoded = DecoderStudio::decode_jwt(jwt)?;
        let mut issues = Vec::new();
        let alg = decoded.algorithm.to_lowercase();
        if alg == "none" {
            issues.push("alg=none: signature not verified (critical)".into());
        }
        if alg.starts_with("hs") {
            issues.push(
                "HMAC algorithm: ensure the secret is high-entropy and not a public key".into(),
            );
        }
        if decoded.signature_raw.is_empty() {
            issues.push("empty signature segment".into());
        }
        let mut expired = None;
        if let Some(payload) = &decoded.payload_json {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(payload) {
                match val.get("exp").and_then(|e| e.as_u64()) {
                    Some(exp) => {
                        let is_exp = exp < now_unix;
                        expired = Some(is_exp);
                        if is_exp {
                            issues.push(format!("token expired at exp={exp} (now={now_unix})"));
                        }
                    }
                    None => issues.push("missing 'exp' claim (token never expires)".into()),
                }
                if val.get("alg").is_some() {
                    issues.push("'alg' present in payload — possible confusion with header".into());
                }
            }
        }
        Ok(JwtDeepReport {
            algorithm: decoded.algorithm,
            header_json: decoded.header_json,
            payload_json: decoded.payload_json,
            issues,
            expired,
        })
    }

    /// Locally verify an HS256 JWT signature against a candidate `secret` using a
    /// pure-Rust HMAC-SHA256 (no network, no external crates). Returns `Ok(true)`
    /// when the recomputed signature matches the token's signature segment.
    pub fn verify_jwt_hmac(&self, jwt: &str, secret: &str) -> Result<bool, String> {
        let parts: Vec<&str> = jwt.trim().split('.').collect();
        if parts.len() != 3 {
            return Err("invalid JWT structure".into());
        }
        let decoded = DecoderStudio::decode_jwt(jwt)?;
        if !decoded.algorithm.eq_ignore_ascii_case("HS256") {
            return Err(format!(
                "local verify supports HS256 only (token alg={})",
                decoded.algorithm
            ));
        }
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let mac = hmac_sha256(secret.as_bytes(), signing_input.as_bytes());
        let expected = base64_url_no_pad(&mac);
        Ok(constant_time_eq(expected.as_bytes(), parts[2].as_bytes()))
    }

    // 5. Promote to Finding -------------------------------------------------
    /// Build a Markdown finding document from observed request/response traces,
    /// reusing the Universal Test Generator's finding exporter.
    pub fn promote_to_finding(
        &self,
        title: &str,
        target: &str,
        traces: Vec<NetworkTrace>,
    ) -> String {
        let mut session = RecordedSession::new(title, target);
        session.traces = traces;
        TestGenerator::export_finding(&session)
    }

    // 6. Surface Crawler ----------------------------------------------------
    /// Collapse discovered (method, url) pairs into parameterized endpoint
    /// templates (numeric / uuid segments become `:id`) and aggregate params.
    pub fn crawl_surface(&self, discovered: &[(String, String)]) -> Vec<SurfaceEndpoint> {
        let mut map: BTreeMap<(String, String), (std::collections::BTreeSet<String>, usize)> =
            BTreeMap::new();
        for (method, url) in discovered {
            let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
            let template = templatize_path(path);
            let entry = map.entry((method.to_uppercase(), template)).or_default();
            for pair in query.split('&').filter(|p| !p.is_empty()) {
                let key = pair.split('=').next().unwrap_or("").to_string();
                if !key.is_empty() {
                    entry.0.insert(key);
                }
            }
            entry.1 += 1;
        }
        map.into_iter()
            .map(
                |((method, path_template), (params, instances))| SurfaceEndpoint {
                    method,
                    path_template,
                    params: params.into_iter().collect(),
                    instances,
                },
            )
            .collect()
    }

    /// Scope-filtered surface crawl: only (method, url) pairs whose host is in
    /// `host_allowlist` (or a subdomain of an allowlisted host) are templatized.
    /// An empty allowlist admits everything.
    pub fn crawl_surface_scoped(
        &self,
        discovered: &[(String, String)],
        host_allowlist: &[String],
    ) -> Vec<SurfaceEndpoint> {
        let in_scope = |url: &str| {
            if host_allowlist.is_empty() {
                return true;
            }
            let host = host_of(url);
            host_allowlist.iter().any(|h| {
                let h = h.to_lowercase();
                host == h || host.ends_with(&format!(".{h}"))
            })
        };
        let filtered: Vec<(String, String)> = discovered
            .iter()
            .filter(|(_, url)| in_scope(url))
            .cloned()
            .collect();
        self.crawl_surface(&filtered)
    }

    // 7. Storage Auditor ----------------------------------------------------
    pub fn audit_storage(&self, items: &[StorageItem]) -> Vec<StorageFinding> {
        let mut findings = Vec::new();
        for item in items {
            let key_l = item.key.to_lowercase();
            let looks_sensitive = [
                "token", "secret", "password", "apikey", "api_key", "jwt", "auth", "session",
                "ssn", "credit",
            ]
            .iter()
            .any(|p| key_l.contains(p));
            let value_is_jwt = DecoderStudio::is_jwt(item.value.trim());
            if (looks_sensitive || value_is_jwt) && item.store != "cookie" {
                findings.push(StorageFinding {
                    key: item.key.clone(),
                    store: item.store.clone(),
                    severity: "high".into(),
                    detail:
                        "sensitive value stored in JS-accessible web storage (XSS-exfiltratable)"
                            .into(),
                });
            }
            if item.store == "cookie" && looks_sensitive {
                if !item.http_only {
                    findings.push(StorageFinding {
                        key: item.key.clone(),
                        store: item.store.clone(),
                        severity: "high".into(),
                        detail: "sensitive cookie missing HttpOnly flag".into(),
                    });
                }
                if !item.secure {
                    findings.push(StorageFinding {
                        key: item.key.clone(),
                        store: item.store.clone(),
                        severity: "medium".into(),
                        detail: "sensitive cookie missing Secure flag".into(),
                    });
                }
                if item
                    .same_site
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("none"))
                    .unwrap_or(true)
                {
                    findings.push(StorageFinding {
                        key: item.key.clone(),
                        store: item.store.clone(),
                        severity: "medium".into(),
                        detail: "cookie SameSite is None/unset (CSRF exposure)".into(),
                    });
                }
            }
        }
        findings
    }

    // 8. Fuzz Repeater ------------------------------------------------------
    pub fn fuzz_payloads(category: FuzzCategory) -> Vec<&'static str> {
        match category {
            FuzzCategory::Sqli => vec![
                "' OR '1'='1",
                "'; DROP TABLE users;--",
                "\" OR 1=1--",
                "1 UNION SELECT NULL--",
            ],
            FuzzCategory::Xss => vec![
                "<script>alert(1)</script>",
                "\"><img src=x onerror=alert(1)>",
                "javascript:alert(1)",
            ],
            FuzzCategory::PathTraversal => vec![
                "../../../../etc/passwd",
                "..%2f..%2f..%2fetc%2fpasswd",
                "....//....//etc/passwd",
            ],
            FuzzCategory::CommandInjection => vec!["; id", "| whoami", "$(id)", "`id`"],
            FuzzCategory::Ssti => vec!["{{7*7}}", "${7*7}", "<%= 7*7 %>", "#{7*7}"],
        }
    }

    /// Build fuzz cases by substituting `FUZZ` in a template with each payload.
    pub fn build_fuzz_cases(&self, template: &str, category: FuzzCategory) -> Vec<FuzzCase> {
        let cat = format!("{category:?}");
        Self::fuzz_payloads(category)
            .into_iter()
            .map(|payload| FuzzCase {
                category: cat.clone(),
                payload: payload.to_string(),
                mutated: template.replace("FUZZ", payload),
            })
            .collect()
    }

    /// Consent-gated fuzzing: refuse to generate active attack payloads against a
    /// target that is not private/local (authorized local testing only, per the
    /// `net/consent` scope boundary). `target_url` is the endpoint under test.
    pub fn build_fuzz_cases_gated(
        &self,
        template: &str,
        category: FuzzCategory,
        target_url: &str,
    ) -> Result<Vec<FuzzCase>, String> {
        if !crate::net::consent::is_private_or_local_url(target_url) {
            return Err(format!(
                "Fuzzing blocked: {target_url} is not private/local (authorized local targets only)"
            ));
        }
        if crate::net::consent::is_blocked_abuse_target(target_url) {
            return Err("Fuzzing blocked: metadata / link-local abuse target".into());
        }
        Ok(self.build_fuzz_cases(template, category))
    }

    // 9. CSRF Analyzer ------------------------------------------------------
    pub fn analyze_csrf(&self, ctx: &CsrfContext) -> CsrfReport {
        let method = ctx.method.to_uppercase();
        let state_changing = matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
        let mut issues = Vec::new();
        if !state_changing {
            return CsrfReport {
                vulnerable: false,
                issues,
            };
        }
        if !ctx.uses_cookie_auth {
            // Non-cookie auth (bearer) is not automatically sent by the browser.
            return CsrfReport {
                vulnerable: false,
                issues,
            };
        }
        let same_site_ok = ctx
            .cookie_same_site
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("lax") || s.eq_ignore_ascii_case("strict"))
            .unwrap_or(false);
        if !ctx.has_csrf_token {
            issues.push(
                "no anti-CSRF token on a state-changing, cookie-authenticated request".into(),
            );
        }
        if !same_site_ok {
            issues.push("session cookie SameSite is not Lax/Strict".into());
        }
        let is_simple_ct = ctx
            .content_type
            .as_deref()
            .map(|c| {
                c.contains("application/x-www-form-urlencoded")
                    || c.contains("multipart/form-data")
                    || c.contains("text/plain")
            })
            .unwrap_or(true);
        if is_simple_ct && !ctx.has_csrf_token {
            issues.push("'simple' content-type allows cross-site form submission".into());
        }
        CsrfReport {
            vulnerable: !issues.is_empty(),
            issues,
        }
    }

    // 10. Sink / Pollution --------------------------------------------------
    /// Detect prototype-pollution keys in a JSON value and dangerous JS sinks in
    /// an optional script snippet.
    pub fn detect_sinks(
        &self,
        json_input: &serde_json::Value,
        script: Option<&str>,
    ) -> Vec<SinkFinding> {
        let mut findings = Vec::new();
        Self::scan_pollution(json_input, "$", &mut findings);
        if let Some(js) = script {
            for sink in [
                "innerHTML",
                "outerHTML",
                "document.write",
                "eval(",
                "setTimeout(",
                "dangerouslySetInnerHTML",
                "insertAdjacentHTML",
            ] {
                if js.contains(sink) {
                    findings.push(SinkFinding {
                        kind: "dom-xss-sink".into(),
                        location: sink.to_string(),
                        severity: if sink == "eval(" {
                            "critical".into()
                        } else {
                            "high".into()
                        },
                        detail: format!(
                            "dangerous sink `{sink}` present; verify input is sanitized"
                        ),
                    });
                }
            }
        }
        findings
    }

    fn scan_pollution(value: &serde_json::Value, path: &str, out: &mut Vec<SinkFinding>) {
        if let serde_json::Value::Object(obj) = value {
            for (k, v) in obj {
                if matches!(k.as_str(), "__proto__" | "constructor" | "prototype") {
                    out.push(SinkFinding {
                        kind: "prototype-pollution".into(),
                        location: format!("{path}.{k}"),
                        severity: "critical".into(),
                        detail: format!("dangerous key `{k}` in object payload"),
                    });
                }
                Self::scan_pollution(v, &format!("{path}.{k}"), out);
            }
        } else if let serde_json::Value::Array(arr) = value {
            for (i, v) in arr.iter().enumerate() {
                Self::scan_pollution(v, &format!("{path}[{i}]"), out);
            }
        }
    }

    // 11. TLS Inspector -----------------------------------------------------
    pub fn inspect_tls(&self, info: &TlsInfo) -> TlsReport {
        let mut issues = Vec::new();
        let proto = info.protocol.to_uppercase().replace(['.', ' '], "");
        let weak_proto = matches!(
            proto.as_str(),
            "TLS10" | "TLS11" | "SSL30" | "SSLV3" | "TLSV10" | "TLSV11"
        );
        if weak_proto {
            issues.push(format!("deprecated protocol {}", info.protocol));
        }
        let cipher_l = info.cipher.to_lowercase();
        if cipher_l.contains("rc4")
            || cipher_l.contains("3des")
            || cipher_l.contains("null")
            || cipher_l.contains("md5")
        {
            issues.push(format!("weak cipher {}", info.cipher));
        }
        if info.self_signed {
            issues.push("self-signed certificate".into());
        }
        if !info.hostname_matches {
            issues.push("certificate hostname mismatch".into());
        }
        if info.cert_days_remaining < 0 {
            issues.push("certificate expired".into());
        } else if info.cert_days_remaining < 14 {
            issues.push(format!(
                "certificate expires in {} days",
                info.cert_days_remaining
            ));
        }
        let grade = if issues.is_empty() {
            if proto == "TLS13" {
                "A+"
            } else {
                "A"
            }
        } else if weak_proto || info.self_signed || info.cert_days_remaining < 0 {
            "F"
        } else {
            "C"
        };
        TlsReport {
            grade: grade.to_string(),
            issues,
        }
    }

    // 12. OOB Listener ------------------------------------------------------
    /// Allocate a unique out-of-band correlation token (subdomain) for blind
    /// SSRF/RCE detection.
    pub fn new_oob_token(&self) -> String {
        let mut c = self.oob_counter.lock();
        *c += 1;
        format!("t{:x}{:x}.{}", now_ms(), *c, self.oob_domain)
    }

    pub fn record_oob(&self, interaction: OobInteraction) {
        self.oob_interactions.lock().push(interaction);
    }

    pub fn oob_interactions(&self, token: &str) -> Vec<OobInteraction> {
        self.oob_interactions
            .lock()
            .iter()
            .filter(|i| i.token == token)
            .cloned()
            .collect()
    }

    pub fn all_oob_interactions(&self) -> Vec<OobInteraction> {
        self.oob_interactions.lock().clone()
    }

    /// Start a real localhost HTTP loopback listener for out-of-band interaction
    /// detection (blind SSRF/RCE). Callbacks are correlated to the OOB token that
    /// appears in the request path or `Host` header. Pass `0` for an ephemeral
    /// port; returns the bound port.
    pub fn start_oob_listener(&self, port: u16) -> Result<u16, String> {
        let mut slot = self.oob_sink.lock();
        if let Some(h) = slot.take() {
            h.running.store(false, Ordering::SeqCst);
        }
        let listener =
            TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind failed: {e}"))?;
        let bound = listener.local_addr().map_err(|e| e.to_string())?.port();
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        std::thread::spawn(move || oob_listener_loop(listener, flag));
        *slot = Some(OobSinkHandle {
            port: bound,
            running,
        });
        Ok(bound)
    }

    pub fn stop_oob_listener(&self) {
        if let Some(h) = self.oob_sink.lock().take() {
            h.running.store(false, Ordering::SeqCst);
        }
    }

    pub fn oob_listener_port(&self) -> Option<u16> {
        self.oob_sink.lock().as_ref().map(|h| h.port)
    }

    /// Build a full promote-to-finding bundle: a Markdown report, a HAR-like JSON
    /// entry list, and a reproducible cURL script — everything needed to file the
    /// finding and reproduce it.
    pub fn promote_to_finding_bundle(
        &self,
        title: &str,
        target: &str,
        traces: Vec<NetworkTrace>,
    ) -> serde_json::Value {
        let markdown = self.promote_to_finding(title, target, traces.clone());
        let mut session = RecordedSession::new(title, target);
        session.traces = traces.clone();
        let curl = TestGenerator::export_curl(&session);
        let entries: Vec<serde_json::Value> = traces
            .iter()
            .map(|t| {
                json!({
                    "request": {
                        "method": t.method,
                        "url": t.url,
                        "headers": t.headers,
                        "postData": t.body,
                    },
                    "response": { "status": t.status },
                })
            })
            .collect();
        let har = json!({
            "log": {
                "version": "1.2",
                "creator": { "name": "Thor", "version": "0.1.1" },
                "entries": entries,
            }
        });
        json!({
            "title": title,
            "target": target,
            "markdown": markdown,
            "curl": curl,
            "har": har,
        })
    }

    // ---- Wave-2 security --------------------------------------------------

    pub fn dom_xss_taint_map(&self, flows: &[TaintFlow]) -> DomXssTaintReport {
        let mut enriched = Vec::with_capacity(flows.len());
        let mut by_sink_kind: BTreeMap<String, usize> = BTreeMap::new();
        for f in flows {
            let mut flow = f.clone();
            let kind = flow
                .sink_kind
                .clone()
                .unwrap_or_else(|| classify_dom_xss_sink(&flow.sink));
            flow.sink_kind = Some(kind.clone());
            *by_sink_kind.entry(kind).or_default() += 1;
            enriched.push(flow);
        }
        let unsanitized = enriched.iter().filter(|f| !f.sanitized).count();
        let risk = if unsanitized == 0 {
            "low"
        } else if unsanitized <= 2 {
            "medium"
        } else {
            "high"
        };
        DomXssTaintReport {
            flows: enriched,
            unsanitized,
            risk: risk.to_string(),
            by_sink_kind,
        }
    }

    /// Parse CDP-shaped taint JSON (`[{source,sink,via,sanitized}]` or
    /// `{flows:[…]}`) into [`TaintFlow`]s for [`dom_xss_taint_map`].
    pub fn parse_dom_xss_taint_flows(json: &str) -> Vec<TaintFlow> {
        let v: serde_json::Value = match serde_json::from_str(json.trim()) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let arr = if let Some(a) = v.as_array() {
            a.clone()
        } else if let Some(a) = v.get("flows").and_then(|f| f.as_array()) {
            a.clone()
        } else {
            return Vec::new();
        };
        arr.iter()
            .filter_map(|item| {
                let source = item.get("source")?.as_str()?.to_string();
                let sink = item.get("sink")?.as_str()?.to_string();
                let via = item
                    .get("via")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let sanitized = item
                    .get("sanitized")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);
                let sink_kind = item
                    .get("sink_kind")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| Some(classify_dom_xss_sink(&sink)));
                Some(TaintFlow {
                    source,
                    sink,
                    via,
                    sanitized,
                    sink_kind,
                })
            })
            .collect()
    }

    pub fn cors_deep_probe(&self, probes: &[CorsProbeResult]) -> CorsDeepReport {
        let mut findings = Vec::new();
        let mut reflects_origin = false;
        let mut credentials_with_star = false;
        let mut credentials_with_reflected_origin = false;
        for p in probes {
            if let Some(acao) = &p.acao {
                if acao == "*" {
                    findings.push(format!("ACAO * for origin {}", p.origin));
                    if p.acac.as_deref() == Some("true") {
                        credentials_with_star = true;
                        findings.push("ACAO * with credentials (invalid/dangerous)".into());
                    }
                } else if acao == &p.origin {
                    reflects_origin = true;
                    if p.origin == "null" {
                        findings.push("reflects Origin: null".into());
                    }
                    if p.acac.as_deref() == Some("true") {
                        credentials_with_reflected_origin = true;
                        findings.push(format!(
                            "reflected origin {} with Access-Control-Allow-Credentials: true",
                            p.origin
                        ));
                    }
                }
            }
        }
        CorsDeepReport {
            findings,
            reflects_origin,
            credentials_with_star,
            credentials_with_reflected_origin,
        }
    }

    pub fn csp_policy_forensics(&self, policy: &str) -> CspForensicsReport {
        let mut directives: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for part in policy.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let mut toks = part.split_whitespace();
            if let Some(name) = toks.next() {
                directives.insert(name.to_lowercase(), toks.map(|s| s.to_string()).collect());
            }
        }
        let mut issues = Vec::new();
        let mut score = 100u32;
        let unsafe_tokens = ["'unsafe-inline'", "'unsafe-eval'", "data:", "*"];
        for (dir, vals) in &directives {
            for v in vals {
                if unsafe_tokens.iter().any(|t| v.contains(t)) {
                    issues.push(format!("{dir} contains {v}"));
                    score = score.saturating_sub(15);
                }
            }
        }
        if !directives.contains_key("default-src") && !directives.contains_key("script-src") {
            issues.push("missing default-src and script-src".into());
            score = score.saturating_sub(25);
        }
        if !directives.contains_key("object-src") {
            issues.push("consider object-src 'none'".into());
            score = score.saturating_sub(5);
        }
        CspForensicsReport {
            directives,
            issues,
            score,
        }
    }

    pub fn oauth_oidc_misconfig(&self, cfg: &OidcConfig) -> OidcMisconfigReport {
        let mut issues: Vec<String> = Vec::new();
        if cfg.authorization_endpoint.starts_with("http://") {
            issues.push("authorization_endpoint not HTTPS".into());
        }
        if cfg.token_endpoint.starts_with("http://") {
            issues.push("token_endpoint not HTTPS".into());
        }
        if cfg.jwks_uri.is_none() {
            issues.push("missing jwks_uri".into());
        }
        if cfg.response_types_supported.iter().any(|r| r == "token") {
            issues.push("implicit flow (response_type=token) enabled".into());
        }
        if cfg
            .grant_types_supported
            .iter()
            .any(|g| g == "password")
        {
            issues.push("resource-owner password credentials grant enabled".into());
        }
        let severity = if issues.iter().any(|i| i.contains("not HTTPS") || i.contains("password")) {
            "high"
        } else if issues.is_empty() {
            "none"
        } else {
            "medium"
        };
        OidcMisconfigReport {
            issues,
            severity: severity.to_string(),
        }
    }

    pub fn cookie_session_forensics(&self, cookies: &[CookieJarEntry]) -> CookieForensicsReport {
        let mut issues = Vec::new();
        let mut session_cookies = 0usize;
        for c in cookies {
            let sessionish = c.name.to_lowercase().contains("session")
                || c.name.to_lowercase().contains("sid")
                || c.max_age.is_none();
            if sessionish {
                session_cookies += 1;
                if !c.secure {
                    issues.push(format!("{} missing Secure", c.name));
                }
                if !c.http_only {
                    issues.push(format!("{} missing HttpOnly", c.name));
                }
                match c.same_site.as_deref().map(|s| s.to_lowercase()) {
                    Some(s) if s == "strict" || s == "lax" || s == "none" => {
                        if s == "none" && !c.secure {
                            issues.push(format!("{} SameSite=None without Secure", c.name));
                        }
                    }
                    _ => issues.push(format!("{} missing/invalid SameSite", c.name)),
                }
            }
            if c.value.len() > 4096 {
                issues.push(format!("{} value exceeds 4KB", c.name));
            }
        }
        CookieForensicsReport {
            session_cookies,
            issues,
        }
    }

    /// Consent-aware: only flags fixation when login succeeds; always requires
    /// `target_url` and gates via `require_lab_consent` before analysis.
    pub fn session_fixation_lab(&self, probe: &SessionFixationProbe) -> Result<SessionFixationReport, String> {
        if probe.target_url.trim().is_empty() {
            return Err("session-fixation-lab requires target_url".into());
        }
        crate::net::consent::require_lab_consent(&probe.target_url, probe.lab_consent)?;
        if probe.login_status < 200 || probe.login_status >= 400 {
            return Ok(SessionFixationReport {
                vulnerable: false,
                detail: format!("login did not succeed (status {})", probe.login_status),
            });
        }
        let vulnerable = !probe.pre_auth_session.is_empty()
            && probe.pre_auth_session == probe.post_auth_session;
        Ok(SessionFixationReport {
            vulnerable,
            detail: if vulnerable {
                "session id unchanged across authentication".into()
            } else {
                "session rotated on login".into()
            },
        })
    }

    pub fn graphql_authz_matrix(&self, probes: &[GraphqlAuthzProbe]) -> GraphqlAuthzReport {
        let mut escalations = 0usize;
        let mut cells = Vec::new();
        for p in probes {
            let allowed = (200..300).contains(&p.status) && p.errors.is_empty();
            let verdict = match (p.should_allow, allowed) {
                (false, true) => {
                    escalations += 1;
                    "PRIVILEGE-ESCALATION"
                }
                (true, false) => "FALSE-DENY",
                (true, true) => "OK-ALLOW",
                (false, false) => "OK-DENY",
            };
            cells.push(format!(
                "{} {} {}: {verdict}",
                p.role, p.operation, p.field
            ));
        }
        GraphqlAuthzReport { escalations, cells }
    }

    pub fn mass_assignment_diff(&self, diff: &MassAssignDiff) -> MassAssignReport {
        let sent: std::collections::HashSet<_> = diff.sent_fields.iter().collect();
        let accepted: std::collections::HashSet<_> = diff.accepted_fields.iter().collect();
        let privileged: std::collections::HashSet<_> = diff.privileged_fields.iter().collect();
        let unexpected: Vec<String> = accepted
            .difference(&sent)
            .map(|s| (*s).clone())
            .collect();
        let privileged_accepted: Vec<String> = accepted
            .intersection(&privileged)
            .map(|s| (*s).clone())
            .collect();
        let risk = if !privileged_accepted.is_empty() {
            "high"
        } else if !unexpected.is_empty() {
            "medium"
        } else {
            "low"
        };
        MassAssignReport {
            unexpected_writes: unexpected,
            privileged_accepted,
            risk: risk.to_string(),
        }
    }

    pub fn race_window_repeater(&self, attempts: &[RaceAttempt]) -> RaceWindowReport {
        let mut outcomes: BTreeMap<(u16, String), usize> = BTreeMap::new();
        let mut min_start = u64::MAX;
        let mut max_end = 0u64;
        for a in attempts {
            *outcomes
                .entry((a.status, a.body_hash.clone()))
                .or_default() += 1;
            min_start = min_start.min(a.start_ms);
            max_end = max_end.max(a.end_ms);
        }
        let unique_outcomes = outcomes.len();
        let window_ms = if attempts.is_empty() {
            0
        } else {
            max_end.saturating_sub(min_start)
        };
        let inconsistency = unique_outcomes > 1;
        RaceWindowReport {
            attempts: attempts.len(),
            unique_outcomes,
            window_ms,
            inconsistency,
            detail: if inconsistency {
                format!("{unique_outcomes} distinct outcomes in {window_ms}ms window")
            } else {
                "consistent outcomes".into()
            },
        }
    }

    /// Consent-gated live parallel fetches → [`race_window_repeater`].
    pub fn race_window_live(
        &self,
        url: &str,
        count: usize,
        lab_consent: bool,
    ) -> Result<RaceWindowReport, String> {
        if url.trim().is_empty() {
            return Err("race-window live requires url".into());
        }
        crate::net::consent::require_lab_consent(url, lab_consent)?;
        let n = count.clamp(2, 32);
        let url = url.to_string();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let attempts = rt.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            let start_wall = std::time::Instant::now();
            let futs: Vec<_> = (0..n)
                .map(|i| {
                    let client = client.clone();
                    let url = url.clone();
                    async move {
                        let t0 = start_wall.elapsed().as_millis() as u64;
                        let (status, body) = match client.get(&url).send().await {
                            Ok(resp) => {
                                let status = resp.status().as_u16();
                                let body = resp.text().await.unwrap_or_default();
                                (status, body)
                            }
                            Err(e) => (0, e.to_string()),
                        };
                        let t1 = start_wall.elapsed().as_millis() as u64;
                        let mut hash: u64 = 0xcbf29ce484222325;
                        for b in body.as_bytes() {
                            hash ^= *b as u64;
                            hash = hash.wrapping_mul(0x100000001b3);
                        }
                        RaceAttempt {
                            id: i as u32,
                            status,
                            body_hash: format!("{hash:016x}"),
                            start_ms: t0,
                            end_ms: t1,
                        }
                    }
                })
                .collect();
            futures::future::join_all(futs).await
        });
        Ok(self.race_window_repeater(&attempts))
    }

    /// Consent-aware SSRF lab: labels blocked vs lab-allowed targets using the
    /// same private/local + abuse-block helpers as fuzz gating.
    /// Returns `Err` when any non-local public target lacks `lab_consent`.
    pub fn ssrf_lab_harness(
        &self,
        targets: &[SsrfTarget],
        lab_consent: bool,
    ) -> Result<SsrfLabReport, String> {
        for t in targets {
            if crate::net::consent::is_blocked_abuse_target(&t.url) {
                continue;
            }
            if !crate::net::consent::is_private_or_local_url(&t.url) && !lab_consent {
                return Err(format!(
                    "SSRF lab refused: public target without lab_consent: {}",
                    t.url
                ));
            }
        }
        let mut blocked = Vec::new();
        let mut allowed_lab = Vec::new();
        let mut issues = Vec::new();
        for t in targets {
            if crate::net::consent::is_blocked_abuse_target(&t.url) {
                blocked.push(t.url.clone());
                issues.push(format!("blocked abuse/metadata target {}", t.url));
                continue;
            }
            if crate::net::consent::is_private_or_local_url(&t.url) {
                allowed_lab.push(t.url.clone());
            } else if lab_consent {
                allowed_lab.push(t.url.clone());
                issues.push(format!("lab-consented public target {}", t.url));
            } else {
                blocked.push(t.url.clone());
                issues.push(format!(
                    "non-local target blocked without explicit lab consent: {}",
                    t.url
                ));
            }
            if let Some(ip) = &t.resolved_ip {
                if ip.starts_with("169.254.") || ip == "0.0.0.0" || ip.starts_with("127.") {
                    issues.push(format!("{} resolved to sensitive IP {ip}", t.url));
                }
            }
        }
        Ok(SsrfLabReport {
            blocked,
            allowed_lab,
            issues,
        })
    }

    pub fn sri_supply_chain_auditor(&self, scripts: &[SriScript]) -> SriAuditReport {
        let mut missing_integrity = Vec::new();
        let mut weak_algo = Vec::new();
        let mut missing_crossorigin = Vec::new();
        for s in scripts {
            match &s.integrity {
                None => missing_integrity.push(s.src.clone()),
                Some(integ) => {
                    let lower = integ.to_lowercase();
                    if lower.starts_with("sha256-") || lower.starts_with("sha384-") || lower.starts_with("sha512-") {
                        // ok
                    } else if lower.starts_with("sha1-") || lower.starts_with("md5-") {
                        weak_algo.push(s.src.clone());
                    } else {
                        weak_algo.push(s.src.clone());
                    }
                    if s.crossorigin.is_none() {
                        missing_crossorigin.push(s.src.clone());
                    }
                }
            }
        }
        SriAuditReport {
            missing_integrity,
            weak_algo,
            missing_crossorigin,
        }
    }

    pub fn websocket_authz_probe(&self, probes: &[WsAuthzProbe]) -> WsAuthzReport {
        let mut escalations = 0usize;
        let mut findings = Vec::new();
        for p in probes {
            match (p.should_allow, p.connected) {
                (false, true) => {
                    escalations += 1;
                    findings.push(format!(
                        "{} connected to {} but should be denied",
                        p.role, p.url
                    ));
                }
                (true, false) => findings.push(format!(
                    "{} denied from {} unexpectedly",
                    p.role, p.url
                )),
                _ => {}
            }
        }
        WsAuthzReport {
            escalations,
            findings,
        }
    }

}

fn templatize_path(path: &str) -> String {
    let segs: Vec<String> = path
        .split('/')
        .map(|s| {
            if s.is_empty() {
                s.to_string()
            } else if s.chars().all(|c| c.is_ascii_digit()) {
                ":id".to_string()
            } else if is_uuid(s) {
                ":uuid".to_string()
            } else if s.len() >= 24 && s.chars().all(|c| c.is_ascii_alphanumeric()) {
                ":token".to_string()
            } else {
                s.to_string()
            }
        })
        .collect();
    segs.join("/")
}

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Extract the lowercased host of a URL (no scheme/port/path).
fn host_of(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    after
        .split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_lowercase()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// URL-safe base64 encoding without padding (JWT signature format).
fn base64_url_no_pad(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(CHARS[(n & 63) as usize] as char);
        }
    }
    out
}

/// Pure-Rust SHA-256 (FIPS 180-4).
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().enumerate().take(16) {
            let j = i * 4;
            *word = u32::from_be_bytes([block[j], block[j + 1], block[j + 2], block[j + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn classify_dom_xss_sink(sink: &str) -> String {
    let s = sink.to_lowercase();
    if s.contains("innerhtml")
        || s.contains("outerhtml")
        || s.contains("insertadjacenthtml")
        || s.contains("document.write")
    {
        "html".into()
    } else if s.contains("eval")
        || s.contains("function(")
        || s.contains("settimeout")
        || s.contains("setinterval")
        || s.contains("script")
    {
        "js".into()
    } else if s.contains("location")
        || s.contains("href")
        || s.contains("src")
        || s.contains("action")
        || s.contains("window.open")
    {
        "url".into()
    } else if s.contains("setattribute") || s.contains("jquery") || s.contains("$(") {
        "dom".into()
    } else {
        "unknown".into()
    }
}

/// Hex-encode bytes (lowercase).
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// HMAC-SHA256 (RFC 2104). Shared by JWT verify + webhook signature vault.
pub(crate) fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block_key = [0u8; 64];
    if key.len() > 64 {
        let hashed = sha256(key);
        block_key[..32].copy_from_slice(&hashed);
    } else {
        block_key[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= block_key[i];
        opad[i] ^= block_key[i];
    }
    let mut inner = Vec::with_capacity(64 + message.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(message);
    let inner_hash = sha256(&inner);
    let mut outer = Vec::with_capacity(96);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

/// Minimal OOB loopback listener loop: records each callback as an
/// [`OobInteraction`] correlated by the token found in the path or Host header.
fn oob_listener_loop(listener: TcpListener, running: Arc<AtomicBool>) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, addr)) => {
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                let (token, path) = oob_token_from_request(&raw);
                AppState::global()
                    .features()
                    .security()
                    .record_oob(OobInteraction {
                        token,
                        protocol: "http".into(),
                        source_ip: addr.ip().to_string(),
                        received_ms: now_ms(),
                        detail: format!("HTTP callback {path}"),
                    });
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
}

/// Extract an OOB correlation token from a raw HTTP request: prefer the first
/// path segment, fall back to the leftmost Host label.
fn oob_token_from_request(raw: &str) -> (String, String) {
    let (head, _) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or("");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();
    let first_seg = path.trim_start_matches('/').split(['/', '?']).next().unwrap_or("");
    if !first_seg.is_empty() {
        return (first_seg.to_string(), path);
    }
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("host") {
                let host = v.trim().split(':').next().unwrap_or("");
                let label = host.split('.').next().unwrap_or("");
                if !label.is_empty() {
                    return (label.to_string(), path);
                }
            }
        }
    }
    ("unknown".to_string(), path)
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
