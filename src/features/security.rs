//! Security specialist features (12).
//!
//! Authorization matrix, response diff, security-header analysis, deep JWT
//! inspection, promote-to-finding, attack-surface crawling, storage auditing,
//! fuzz repeater, CSRF analysis, sink/pollution detection, TLS inspection, and
//! an out-of-band interaction listener. Reuses [`crate::dx::decoder`] and
//! [`crate::dx::test_generator`] where appropriate.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

/// Security specialist engine.
pub struct SecurityEngine {
    oob_domain: String,
    oob_counter: Mutex<u64>,
    oob_interactions: Mutex<Vec<OobInteraction>>,
}

impl Default for SecurityEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityEngine {
    pub fn new() -> Self {
        Self {
            oob_domain: "oob.thor.local".to_string(),
            oob_counter: Mutex::new(0),
            oob_interactions: Mutex::new(Vec::new()),
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
}
