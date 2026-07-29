//! Test / QA specialist features (12).
//!
//! Resilient locators, flake radar, visual regression, identity fixtures,
//! data-driven scenarios, real vitals gates, contract testing, deflaked export,
//! CI bundle, time-travel trace, determinism harness, and authz-matrix test
//! generation. Reuses [`crate::dx::test_generator`] and [`crate::load`].

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::dx::test_generator::{ExportFormat, LocatorCandidate, RecordedSession, TestGenerator};
use crate::load::{BrowserMetrics, PerformanceBudgets};

// ---------------------------------------------------------------------------
// 1. Resilient Locators
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElementSignals {
    #[serde(default)]
    pub test_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub accessible_name: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub css: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedLocator {
    pub expression: String,
    pub strategy: String,
    pub resilience_score: u32,
}

// ---------------------------------------------------------------------------
// 2. Flake Radar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakeReport {
    pub test: String,
    pub runs: usize,
    pub pass_rate: f64,
    /// Fraction of adjacent run pairs that flipped pass<->fail.
    pub flip_rate: f64,
    pub classification: String,
}

// ---------------------------------------------------------------------------
// 3. Visual Regression
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualDiff {
    pub pixel_diff_ratio: f64,
    pub perceptual_distance: u32,
    pub passed: bool,
}

// ---------------------------------------------------------------------------
// 4. Identity Fixtures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdentityFixture {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub cookies: HashMap<String, String>,
    #[serde(default)]
    pub tokens: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// 6. Real Vitals Gates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalsGateResult {
    pub passed: bool,
    pub failures: Vec<String>,
}

// ---------------------------------------------------------------------------
// 7. Contract Testing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInteraction {
    pub description: String,
    pub method: String,
    pub path: String,
    pub expected_status: u16,
    #[serde(default)]
    pub response_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualInteraction {
    pub method: String,
    pub path: String,
    pub status: u16,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractResult {
    pub satisfied: bool,
    pub failures: Vec<String>,
    pub verified: usize,
}

// ---------------------------------------------------------------------------
// 10. Time-Travel Trace
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceFrame {
    pub index: usize,
    pub step: String,
    pub action: String,
    pub dom_digest: String,
    pub timestamp_ms: u64,
}

// ---------------------------------------------------------------------------
// 11. Determinism Harness
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterminismReport {
    pub runs: usize,
    pub distinct_outputs: usize,
    pub deterministic: bool,
    pub majority_ratio: f64,
}

// ---------------------------------------------------------------------------
// 12. Authz Matrix Tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzTestCase {
    pub name: String,
    pub role: String,
    pub endpoint: String,
    pub expected_status_class: String, // "2xx" | "4xx"
}

/// QA specialist engine.
pub struct QaEngine {
    fixtures: Mutex<HashMap<String, IdentityFixture>>,
    traces: Mutex<HashMap<String, Vec<TraceFrame>>>,
}

impl Default for QaEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl QaEngine {
    pub fn new() -> Self {
        Self {
            fixtures: Mutex::new(HashMap::new()),
            traces: Mutex::new(HashMap::new()),
        }
    }

    // 1. Resilient Locators -------------------------------------------------
    /// Rank locator strategies by resilience (test-id > role+name > label >
    /// text > id > css) and return the ordered candidate list.
    pub fn rank_locators(&self, signals: &ElementSignals) -> Vec<RankedLocator> {
        let mut ranked = Vec::new();
        let mut push = |expr: Option<String>, strategy: &str, score: u32| {
            if let Some(e) = expr {
                ranked.push(RankedLocator {
                    expression: e,
                    strategy: strategy.to_string(),
                    resilience_score: score,
                });
            }
        };
        let cand = LocatorCandidate {
            test_id: signals.test_id.clone(),
            role: signals.role.clone(),
            name: signals.accessible_name.clone(),
            label: signals.label.clone(),
            text: signals.text.clone(),
            css: signals.css.clone(),
        };
        push(
            cand.test_id.as_ref().and_then(|_| {
                LocatorCandidate {
                    test_id: cand.test_id.clone(),
                    ..Default::default()
                }
                .to_playwright()
            }),
            "test-id",
            100,
        );
        if signals.role.is_some() {
            let rc = LocatorCandidate {
                role: cand.role.clone(),
                name: cand.name.clone(),
                ..Default::default()
            };
            push(
                rc.to_playwright(),
                "role",
                if signals.accessible_name.is_some() {
                    90
                } else {
                    70
                },
            );
        }
        push(
            signals.label.as_ref().and_then(|_| {
                LocatorCandidate {
                    label: cand.label.clone(),
                    ..Default::default()
                }
                .to_playwright()
            }),
            "label",
            80,
        );
        push(
            signals.text.as_ref().and_then(|_| {
                LocatorCandidate {
                    text: cand.text.clone(),
                    ..Default::default()
                }
                .to_playwright()
            }),
            "text",
            60,
        );
        if let Some(id) = &signals.id {
            push(Some(format!("page.locator('#{id}')")), "id", 50);
        }
        push(
            signals.css.as_ref().and_then(|_| {
                LocatorCandidate {
                    css: cand.css.clone(),
                    ..Default::default()
                }
                .to_playwright()
            }),
            "css",
            20,
        );
        ranked.sort_by(|a, b| b.resilience_score.cmp(&a.resilience_score));
        ranked
    }

    // 2. Flake Radar --------------------------------------------------------
    /// `results` is an ordered run history (true = pass).
    pub fn flake_radar(&self, test: &str, results: &[bool]) -> FlakeReport {
        let runs = results.len();
        if runs == 0 {
            return FlakeReport {
                test: test.to_string(),
                runs: 0,
                pass_rate: 0.0,
                flip_rate: 0.0,
                classification: "no-data".into(),
            };
        }
        let passes = results.iter().filter(|r| **r).count();
        let pass_rate = passes as f64 / runs as f64;
        let flips = results.windows(2).filter(|w| w[0] != w[1]).count();
        let flip_rate = if runs > 1 {
            flips as f64 / (runs - 1) as f64
        } else {
            0.0
        };
        let classification = if pass_rate == 1.0 {
            "stable-pass"
        } else if pass_rate == 0.0 {
            "consistently-failing"
        } else if flip_rate >= 0.3 {
            "flaky"
        } else {
            "intermittent"
        };
        FlakeReport {
            test: test.to_string(),
            runs,
            pass_rate,
            flip_rate,
            classification: classification.to_string(),
        }
    }

    // 3. Visual Regression --------------------------------------------------
    /// Compare two equal-length pixel buffers (grayscale bytes) plus their dHash
    /// perceptual fingerprints. Returns pixel diff ratio, Hamming distance and a
    /// pass/fail against `threshold` (max allowed pixel diff ratio).
    pub fn visual_regression(
        &self,
        baseline: &[u8],
        candidate: &[u8],
        threshold: f64,
    ) -> VisualDiff {
        let len = baseline.len().min(candidate.len()).max(1);
        let diff_pixels = baseline
            .iter()
            .zip(candidate.iter())
            .filter(|(a, b)| a.abs_diff(**b) > 8)
            .count();
        let size_penalty = baseline.len().abs_diff(candidate.len());
        let pixel_diff_ratio = (diff_pixels + size_penalty) as f64 / len as f64;
        let perceptual_distance = (dhash(baseline) ^ dhash(candidate)).count_ones();
        VisualDiff {
            pixel_diff_ratio,
            perceptual_distance,
            passed: pixel_diff_ratio <= threshold,
        }
    }

    // 4. Identity Fixtures --------------------------------------------------
    pub fn add_fixture(&self, fixture: IdentityFixture) {
        self.fixtures.lock().insert(fixture.name.clone(), fixture);
    }

    pub fn fixture(&self, name: &str) -> Option<IdentityFixture> {
        self.fixtures.lock().get(name).cloned()
    }

    /// Emit a Playwright `storageState`-style setup snippet for a fixture.
    pub fn fixture_setup_script(&self, name: &str) -> Option<String> {
        let f = self.fixture(name)?;
        let cookies: Vec<serde_json::Value> = f
            .cookies
            .iter()
            .map(|(k, v)| serde_json::json!({ "name": k, "value": v }))
            .collect();
        let origins: Vec<serde_json::Value> = if f.tokens.is_empty() {
            vec![]
        } else {
            vec![serde_json::json!({
                "origin": "",
                "localStorage": f.tokens.iter().map(|(k, v)| serde_json::json!({ "name": k, "value": v })).collect::<Vec<_>>()
            })]
        };
        let state = serde_json::json!({ "cookies": cookies, "origins": origins });
        Some(serde_json::to_string_pretty(&state).unwrap_or_default())
    }

    // 5. Data-Driven Scenarios ---------------------------------------------
    /// Expand a `{{col}}` template against dataset rows into concrete cases.
    pub fn expand_data_driven(
        &self,
        template: &str,
        dataset: &[HashMap<String, String>],
    ) -> Vec<String> {
        dataset
            .iter()
            .map(|row| {
                let mut out = template.to_string();
                for (k, v) in row {
                    out = out.replace(&format!("{{{{{k}}}}}"), v);
                }
                out
            })
            .collect()
    }

    // 6. Real Vitals Gates --------------------------------------------------
    pub fn vitals_gate(
        &self,
        vitals: &BrowserMetrics,
        budgets: &PerformanceBudgets,
    ) -> VitalsGateResult {
        let mut failures = Vec::new();
        let mut gate = |name: &str, actual: f64, budget: Option<f64>| {
            if let Some(b) = budget {
                if actual > b {
                    failures.push(format!("{name} {actual:.1} > budget {b:.1}"));
                }
            }
        };
        gate("LCP", vitals.lcp_ms, budgets.max_lcp_ms);
        gate("INP", vitals.inp_ms, budgets.max_inp_ms);
        gate("CLS", vitals.cls, budgets.max_cls);
        gate("TBT", vitals.tbt_ms, budgets.max_tbt_ms);
        gate("TTFB", vitals.ttfb_ms, budgets.max_ttfb_ms);
        VitalsGateResult {
            passed: failures.is_empty(),
            failures,
        }
    }

    // 7. Contract Testing ---------------------------------------------------
    pub fn verify_contract(
        &self,
        interactions: &[ContractInteraction],
        actuals: &[ActualInteraction],
    ) -> ContractResult {
        let mut failures = Vec::new();
        let mut verified = 0;
        for expected in interactions {
            let matched = actuals.iter().find(|a| {
                a.method.eq_ignore_ascii_case(&expected.method) && a.path == expected.path
            });
            match matched {
                None => failures.push(format!(
                    "no actual interaction for {} {}",
                    expected.method, expected.path
                )),
                Some(actual) => {
                    verified += 1;
                    if actual.status != expected.expected_status {
                        failures.push(format!(
                            "{} {}: status {} != expected {}",
                            expected.method, expected.path, actual.status, expected.expected_status
                        ));
                    }
                    if let (Some(schema), Some(body)) = (&expected.response_schema, &actual.body) {
                        let errs = crate::features::backend::BackendEngine::new()
                            .validate_contract(body, schema);
                        for e in errs {
                            failures.push(format!("{} {}: {}", expected.method, expected.path, e));
                        }
                    }
                }
            }
        }
        ContractResult {
            satisfied: failures.is_empty(),
            failures,
            verified,
        }
    }

    // 8. Deflaked Export ----------------------------------------------------
    /// Analyze an existing Playwright test string for brittle patterns.
    pub fn detect_brittle_patterns(&self, test_source: &str) -> Vec<String> {
        let mut issues = Vec::new();
        if test_source.contains("waitForTimeout") {
            issues.push("uses waitForTimeout (fixed sleep) — prefer web-first assertions".into());
        }
        if test_source.contains(":nth-child") || test_source.contains(":nth-of-type") {
            issues.push("positional nth-child selector — brittle to DOM reordering".into());
        }
        if test_source.contains("page.locator('xpath=") || test_source.contains("//div") {
            issues.push("XPath locator — brittle; prefer role/test-id".into());
        }
        if test_source.contains("sleep(") {
            issues.push("raw sleep() — replace with expect(...).toBeVisible()".into());
        }
        issues
    }

    /// Produce a deflaked Playwright test from a recorded session (web-first).
    pub fn export_deflaked(&self, session: &RecordedSession) -> String {
        TestGenerator::promote_to_test(session, ExportFormat::PlaywrightTs)
    }

    // 9. CI Bundle ----------------------------------------------------------
    pub fn export_ci_bundle(&self, session: &RecordedSession) -> String {
        TestGenerator::promote_to_test(session, ExportFormat::CiBundle)
    }

    // 10. Time-Travel Trace -------------------------------------------------
    pub fn record_trace_frame(&self, test: &str, step: &str, action: &str, dom: &str) -> usize {
        let mut traces = self.traces.lock();
        let frames = traces.entry(test.to_string()).or_default();
        let index = frames.len();
        frames.push(TraceFrame {
            index,
            step: step.to_string(),
            action: action.to_string(),
            dom_digest: format!("{:016x}", dhash(dom.as_bytes())),
            timestamp_ms: now_ms(),
        });
        index
    }

    pub fn trace_frames(&self, test: &str) -> Vec<TraceFrame> {
        self.traces.lock().get(test).cloned().unwrap_or_default()
    }

    pub fn trace_frame_at(&self, test: &str, index: usize) -> Option<TraceFrame> {
        self.traces
            .lock()
            .get(test)
            .and_then(|f| f.get(index).cloned())
    }

    // 11. Determinism Harness ----------------------------------------------
    /// Given output fingerprints across repeated runs, report determinism.
    pub fn determinism(&self, outputs: &[String]) -> DeterminismReport {
        let runs = outputs.len();
        if runs == 0 {
            return DeterminismReport {
                runs: 0,
                distinct_outputs: 0,
                deterministic: true,
                majority_ratio: 1.0,
            };
        }
        let mut counts: HashMap<&String, usize> = HashMap::new();
        for o in outputs {
            *counts.entry(o).or_default() += 1;
        }
        let distinct = counts.len();
        let majority = counts.values().copied().max().unwrap_or(0);
        DeterminismReport {
            runs,
            distinct_outputs: distinct,
            deterministic: distinct == 1,
            majority_ratio: majority as f64 / runs as f64,
        }
    }

    // 12. Authz Matrix Tests ------------------------------------------------
    /// Generate authorization test cases from (role, endpoint, should_allow).
    pub fn generate_authz_tests(&self, matrix: &[(String, String, bool)]) -> Vec<AuthzTestCase> {
        matrix
            .iter()
            .map(|(role, endpoint, allow)| AuthzTestCase {
                name: format!(
                    "{role} {} {endpoint}",
                    if *allow { "can access" } else { "is denied" }
                ),
                role: role.clone(),
                endpoint: endpoint.clone(),
                expected_status_class: if *allow { "2xx".into() } else { "4xx".into() },
            })
            .collect()
    }

    /// Render generated authz tests as a Playwright spec.
    pub fn authz_tests_to_playwright(&self, base_url: &str, cases: &[AuthzTestCase]) -> String {
        let mut out = String::new();
        out.push_str("import { test, expect } from '@playwright/test';\n\n");
        for c in cases {
            out.push_str(&format!(
                "test('{}', async ({{ request }}) => {{\n",
                c.name.replace('\'', "\\'")
            ));
            out.push_str(&format!(
                "  const res = await request.get('{base_url}{}');\n",
                c.endpoint
            ));
            if c.expected_status_class == "2xx" {
                out.push_str("  expect(res.status()).toBeLessThan(300);\n");
            } else {
                out.push_str("  expect(res.status()).toBeGreaterThanOrEqual(400);\n");
            }
            out.push_str("});\n\n");
        }
        out
    }
}

/// 64-bit difference hash of a byte buffer (order-sensitive perceptual proxy).
fn dhash(data: &[u8]) -> u64 {
    if data.len() < 2 {
        return 0;
    }
    // Sample 65 evenly spaced points, emit a bit per adjacent increase.
    let mut bits: u64 = 0;
    let n = data.len();
    let mut prev = data[0];
    for i in 0..64 {
        let idx = ((i + 1) * (n - 1)) / 64;
        let cur = data[idx];
        if cur >= prev {
            bits |= 1 << i;
        }
        prev = cur;
    }
    bits
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
    use crate::dx::test_generator::NetworkTrace;

    #[test]
    fn test_rank_locators_prefers_testid() {
        let e = QaEngine::new();
        let signals = ElementSignals {
            test_id: Some("submit".into()),
            role: Some("button".into()),
            accessible_name: Some("Submit".into()),
            css: Some(".btn.primary".into()),
            ..Default::default()
        };
        let ranked = e.rank_locators(&signals);
        assert_eq!(ranked[0].strategy, "test-id");
        assert!(ranked[0].expression.contains("getByTestId"));
        assert!(ranked.iter().any(|r| r.strategy == "role"));
    }

    #[test]
    fn test_flake_radar_classification() {
        let e = QaEngine::new();
        let flaky = e.flake_radar("t", &[true, false, true, false, true]);
        assert_eq!(flaky.classification, "flaky");
        let stable = e.flake_radar("t", &[true, true, true]);
        assert_eq!(stable.classification, "stable-pass");
    }

    #[test]
    fn test_visual_regression() {
        let e = QaEngine::new();
        let base = vec![10u8; 128];
        let same = base.clone();
        assert!(e.visual_regression(&base, &same, 0.01).passed);
        let mut changed = base.clone();
        for b in changed.iter_mut().take(64) {
            *b = 200;
        }
        let diff = e.visual_regression(&base, &changed, 0.1);
        assert!(!diff.passed);
        assert!(diff.pixel_diff_ratio > 0.1);
    }

    #[test]
    fn test_identity_fixture_setup() {
        let e = QaEngine::new();
        let mut f = IdentityFixture {
            name: "admin".into(),
            role: "admin".into(),
            ..Default::default()
        };
        f.cookies.insert("session".into(), "abc".into());
        f.tokens.insert("jwt".into(), "xyz".into());
        e.add_fixture(f);
        let script = e.fixture_setup_script("admin").unwrap();
        assert!(script.contains("session"));
        assert!(script.contains("localStorage"));
    }

    #[test]
    fn test_data_driven_expansion() {
        let e = QaEngine::new();
        let mut r1 = HashMap::new();
        r1.insert("user".to_string(), "alice".to_string());
        let mut r2 = HashMap::new();
        r2.insert("user".to_string(), "bob".to_string());
        let out = e.expand_data_driven("login as {{user}}", &[r1, r2]);
        assert_eq!(out, vec!["login as alice", "login as bob"]);
    }

    #[test]
    fn test_vitals_gate() {
        let e = QaEngine::new();
        let budgets = PerformanceBudgets::core_web_vitals();
        let bad = BrowserMetrics {
            lcp_ms: 5000.0,
            ..Default::default()
        };
        let r = e.vitals_gate(&bad, &budgets);
        assert!(!r.passed);
        assert!(r.failures.iter().any(|f| f.starts_with("LCP")));
    }

    #[test]
    fn test_contract_verification() {
        let e = QaEngine::new();
        let interactions = vec![ContractInteraction {
            description: "get user".into(),
            method: "GET".into(),
            path: "/users/1".into(),
            expected_status: 200,
            response_schema: Some(serde_json::json!({"type": "object", "required": ["id"]})),
        }];
        let actuals = vec![ActualInteraction {
            method: "GET".into(),
            path: "/users/1".into(),
            status: 200,
            body: Some(serde_json::json!({"id": 1})),
        }];
        assert!(e.verify_contract(&interactions, &actuals).satisfied);
        let bad = vec![ActualInteraction {
            method: "GET".into(),
            path: "/users/1".into(),
            status: 404,
            body: None,
        }];
        assert!(!e.verify_contract(&interactions, &bad).satisfied);
    }

    #[test]
    fn test_detect_brittle_patterns() {
        let e = QaEngine::new();
        let src =
            "await page.waitForTimeout(1000); await page.locator('div:nth-child(3)').click();";
        let issues = e.detect_brittle_patterns(src);
        assert!(issues.iter().any(|i| i.contains("waitForTimeout")));
        assert!(issues.iter().any(|i| i.contains("nth-child")));
    }

    #[test]
    fn test_time_travel_trace() {
        let e = QaEngine::new();
        e.record_trace_frame("login", "fill username", "fill", "<input>");
        e.record_trace_frame("login", "click submit", "click", "<button>");
        assert_eq!(e.trace_frames("login").len(), 2);
        assert_eq!(e.trace_frame_at("login", 1).unwrap().action, "click");
    }

    #[test]
    fn test_determinism_harness() {
        let e = QaEngine::new();
        let det = e.determinism(&["a".into(), "a".into(), "a".into()]);
        assert!(det.deterministic);
        let nondet = e.determinism(&["a".into(), "b".into(), "a".into()]);
        assert!(!nondet.deterministic);
        assert_eq!(nondet.distinct_outputs, 2);
        assert!((nondet.majority_ratio - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_authz_test_generation() {
        let e = QaEngine::new();
        let matrix = vec![
            ("admin".to_string(), "/admin".to_string(), true),
            ("user".to_string(), "/admin".to_string(), false),
        ];
        let cases = e.generate_authz_tests(&matrix);
        assert_eq!(cases.len(), 2);
        let spec = e.authz_tests_to_playwright("https://x", &cases);
        assert!(spec.contains("toBeLessThan(300)"));
        assert!(spec.contains("toBeGreaterThanOrEqual(400)"));
    }

    #[test]
    fn test_ci_bundle_and_deflaked() {
        let e = QaEngine::new();
        let mut session = RecordedSession::new("Flow", "https://x");
        session.add_trace(NetworkTrace {
            method: "GET".into(),
            url: "https://x/a".into(),
            headers: HashMap::new(),
            body: None,
            status: 200,
        });
        assert!(e.export_ci_bundle(&session).contains("playwright"));
        assert!(!e.export_deflaked(&session).contains("waitForTimeout"));
    }
}
