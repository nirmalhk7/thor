//! Test / QA specialist features (24).
//!
//! Wave 1: resilient locators, flake radar, visual regression, identity
//! fixtures, data-driven scenarios, real vitals gates, contract testing,
//! deflaked export, CI bundle, time-travel trace, determinism harness, and
//! authz-matrix test generation.
//!
//! Wave 2: flake triager, quarantine policy, contract drift gate, hermetic
//! fixture pack, a11y test gen, snapshot strategy pack, coverage adjacency,
//! parallel shard planner, trace property miner, runtime self-heal, oracle
//! synthesizer, and mutation hook bridge.
//!
//! Reuses [`crate::dx::test_generator`] and [`crate::load`].

mod a11y;
mod contracts;
mod coverage;
mod data;
mod exports;
mod fixtures;
mod flakes;
mod healing;
mod hermetic;
mod locators;
mod oracles;
mod properties;
mod quarantine;
mod shards;
mod signals;
mod snapshots;
mod traces;
mod visual;
mod vitals;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::dx::test_generator::{
    ExportFormat, LocatorCandidate, NetworkTrace, RecordedSession, TestGenerator,
};
use crate::load::{BrowserMetrics, PerformanceBudgets, ProtocolRunner, ThorScenario};

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
    /// Number of pixels excluded from comparison by `mask_regions`.
    #[serde(default)]
    pub masked_pixels: usize,
}

/// Rectangular pixel region to exclude from a visual-regression comparison,
/// resolved from a selector's bounding box (row-major, `width`-strided buffer).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaskRegion {
    /// Source selector this mask was resolved from (for provenance).
    #[serde(default)]
    pub selector: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
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
    #[serde(
        default,
        deserialize_with = "deserialize_json_body",
        skip_serializing_if = "Option::is_none"
    )]
    pub body: Option<serde_json::Value>,
}

fn deserialize_json_body<'de, D>(deserializer: D) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde_json::Value::deserialize(deserializer).map(Some)
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
    /// Full captured DOM markup for this frame (enables time-travel replay, not
    /// just change detection via the digest).
    #[serde(default)]
    pub dom_snapshot: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaywrightTraceExport {
    pub test: String,
    pub dir: String,
    pub frame_count: usize,
    pub frame_paths: Vec<String>,
    pub meta_path: String,
    pub open_hint: String,
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

/// Outcome of replaying a scenario N times to surface flakiness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakeReplayReport {
    pub scenario: String,
    pub requested_runs: usize,
    /// Per-run pass (true) / fail (false) in execution order.
    pub results: Vec<bool>,
    pub errors: Vec<String>,
    /// Underlying flake classification computed from `results`.
    pub radar: FlakeReport,
}

/// Portable CI bundle manifest: the generated workflow plus the artifact paths a
/// pipeline is expected to produce/consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiBundleManifest {
    pub workflow: String,
    pub spec_path: String,
    pub junit_path: String,
    pub har_path: String,
    /// Composed Playwright export (deflaked + oracles when observation present).
    #[serde(default)]
    pub playwright_ts: String,
    /// Hermetic fixture pack bootstrap (storage state + HAR mocks).
    #[serde(default)]
    pub hermetic_ts: String,
    /// Axe a11y scaffold for the session target.
    #[serde(default)]
    pub a11y_scaffold: String,
    /// CI quarantine grep-invert filter snippet.
    #[serde(default)]
    pub quarantine_filter: String,
}

/// CDP emulation script bundle that pins non-deterministic browser inputs
/// (clock, RNG seed, timezone, locale, reduced motion) for reproducible runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterminismControls {
    /// `Emulation.*` / `Runtime.evaluate` CDP command payloads (JSON).
    pub cdp_commands: Vec<serde_json::Value>,
    /// A single injectable JS bootstrap applied before page scripts run.
    pub init_script: String,
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

// ---------------------------------------------------------------------------
// Wave 2 — flake triage, quarantine, drift, fixtures, a11y, snapshots, …
// ---------------------------------------------------------------------------

/// Signal vector used by the flake triager to attribute root cause.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlakeVector {
    #[serde(default)]
    pub timing_cv: f64,
    #[serde(default)]
    pub locator_churn: f64,
    #[serde(default)]
    pub race_score: f64,
    #[serde(default)]
    pub network_jitter_ms: f64,
    #[serde(default)]
    pub animation_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakeTriage {
    pub test: String,
    pub root_cause: String,
    pub confidence: f64,
    pub evidence_frame_indexes: Vec<usize>,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineInput {
    pub test: String,
    pub flake_rate: f64,
    pub consecutive_fails: usize,
    pub days_in_quarantine: f64,
    pub green_streak: usize,
    #[serde(default)]
    pub currently_quarantined: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineDecision {
    pub test: String,
    pub action: String, // quarantine | skip | expire | readmit | keep
    pub reason: String,
    pub quarantine_yaml: String,
    /// Whether `persist_quarantine` wrote `quarantine.yaml` successfully.
    #[serde(default)]
    pub persist_ok: bool,
    /// Persist failure detail when `persist_ok` is false.
    #[serde(default)]
    pub persist_err: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDriftChange {
    pub path: String,
    pub kind: String, // added | removed | status-changed | schema-breaking
    pub detail: String,
    pub breaking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDriftReport {
    pub breaking: bool,
    pub changes: Vec<ContractDriftChange>,
    pub baseline_count: usize,
    pub live_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HarMockEntry {
    pub method: String,
    pub url: String,
    pub status: u16,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageSeed {
    pub origin: String,
    #[serde(default)]
    pub local_storage: HashMap<String, String>,
    #[serde(default)]
    pub session_storage: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermeticFixturePack {
    pub name: String,
    pub identity: IdentityFixture,
    pub har_mocks: Vec<HarMockEntry>,
    pub storage: Vec<StorageSeed>,
    pub pack_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11yViolation {
    pub rule_id: String,
    pub selector: String,
    pub impact: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11ySpec {
    pub playwright_ts: String,
    pub axe_assertions: Vec<String>,
    pub violation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotLayer {
    pub name: String,
    pub strategy: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub name: String,
    pub layers: Vec<SnapshotLayer>,
    pub manifesto_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CoverageEdge {
    pub kind: String, // route | component | api
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageAdjacencyReport {
    pub observed: usize,
    pub covered: usize,
    pub uncovered: Vec<CoverageEdge>,
    pub coverage_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardScenario {
    pub name: String,
    pub duration_ms: f64,
    #[serde(default)]
    pub identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardBucket {
    pub shard_id: usize,
    pub scenarios: Vec<String>,
    pub total_ms: f64,
    #[serde(default)]
    pub identities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardPlan {
    pub shard_count: usize,
    pub shards: Vec<ShardBucket>,
    pub plan_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyCase {
    pub name: String,
    pub kind: String, // idempotency | pagination | authz | property
    pub description: String,
    pub seed_input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealSignal {
    pub locator: String,
    pub strategy: String,
    pub successes: u32,
    pub failures: u32,
    #[serde(default)]
    pub last_error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealPatch {
    pub preferred: RankedLocator,
    pub ranked: Vec<RankedLocator>,
    pub patch_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OracleObservation {
    #[serde(default)]
    pub dom_digest: Option<String>,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub vitals: Option<BrowserMetrics>,
    #[serde(default)]
    pub storage: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleSpec {
    pub kind: String,
    pub assertion: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoveredStep {
    pub id: String,
    pub file: String,
    #[serde(default)]
    pub mutation_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationHookManifest {
    pub stryker: serde_json::Value,
    pub cargo_mutants: serde_json::Value,
    pub covered_steps: usize,
}

/// QA specialist engine.
pub struct QaEngine {
    fixtures: Mutex<HashMap<String, IdentityFixture>>,
    traces: Mutex<HashMap<String, Vec<TraceFrame>>>,
    quarantine: Mutex<HashMap<String, QuarantineDecision>>,
}

impl Default for QaEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl QaEngine {
    pub fn new() -> Self {
        let engine = Self {
            fixtures: Mutex::new(HashMap::new()),
            traces: Mutex::new(HashMap::new()),
            quarantine: Mutex::new(HashMap::new()),
        };
        engine.hydrate_quarantine();
        engine
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
    // NetworkTrace is imported at module level.

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
    fn test_export_playwright_trace_writes_dir() {
        let tmp = std::env::temp_dir().join(format!("thor-trace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("THOR_DATA_DIR", &tmp);

        let e = QaEngine::new();
        e.record_trace_frame("live", "step1", "nav", "<html>one</html>");
        e.record_trace_frame("live", "step2", "click", "<html>two</html>");
        let exp = e.export_playwright_trace("live").expect("export");
        assert_eq!(exp.frame_count, 2);
        assert!(std::path::Path::new(&exp.meta_path).exists());
        assert!(std::path::Path::new(&exp.dir)
            .join("frame-0000.html")
            .exists());
        let _ = std::fs::remove_dir_all(&tmp);
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
    fn test_visual_regression_masked() {
        let e = QaEngine::new();
        // 4x4 grayscale; change only the top-left 2x2, then mask it out.
        let base = vec![10u8; 16];
        let mut cand = base.clone();
        for row in 0..2 {
            for col in 0..2 {
                cand[row * 4 + col] = 200;
            }
        }
        let masks = vec![MaskRegion {
            selector: ".clock".into(),
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        }];
        let masked = e.visual_regression_masked(&base, &cand, 4, 0.01, &masks);
        assert_eq!(masked.masked_pixels, 4);
        assert!(masked.passed, "masked diff should pass: {masked:?}");
        // Without the mask the same change fails.
        let unmasked = e.visual_regression(&base, &cand, 0.01);
        assert!(!unmasked.passed);
    }

    #[test]
    fn test_ci_bundle_manifest() {
        let e = QaEngine::new();
        let session = RecordedSession::new("Checkout Flow", "https://shop");
        let m = e.export_ci_bundle_manifest(&session);
        assert!(m.workflow.contains("playwright"));
        assert!(m.spec_path.contains("checkout-flow"));
        assert!(m.junit_path.ends_with(".junit.xml"));
        assert!(m.har_path.ends_with(".har"));
        assert!(!m.playwright_ts.trim().is_empty());
        assert!(!m.hermetic_ts.trim().is_empty());
        assert!(!m.quarantine_filter.trim().is_empty());
    }

    #[test]
    fn test_trace_frame_stores_dom_snapshot() {
        let e = QaEngine::new();
        e.record_trace_frame("t", "s", "click", "<button>Go</button>");
        let frame = e.trace_frame_at("t", 0).unwrap();
        assert_eq!(frame.dom_snapshot, "<button>Go</button>");
        assert!(!frame.dom_digest.is_empty());
    }

    #[test]
    fn test_determinism_controls() {
        let e = QaEngine::new();
        let c = e.determinism_controls(7, 1000);
        assert!(c.init_script.contains("Math.random"));
        assert!(c.init_script.contains("7"));
        assert!(c
            .cdp_commands
            .iter()
            .any(|cmd| cmd["method"] == "Emulation.setTimezoneOverride"));
        assert!(c
            .cdp_commands
            .iter()
            .any(|cmd| cmd["method"] == "Page.addScriptToEvaluateOnNewDocument"));
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

    #[test]
    fn test_flake_triager() {
        let e = QaEngine::new();
        let radar = e.flake_radar("login", &[true, false, true, false]);
        let vectors = FlakeVector {
            locator_churn: 0.9,
            timing_cv: 0.1,
            ..Default::default()
        };
        e.record_trace_frame("login", "a", "click", "<a/>");
        e.record_trace_frame("login", "b", "click", "<b/>");
        let frames = e.trace_frames("login");
        let triage = e.triage_flake("login", &radar, &vectors, &frames);
        assert_eq!(triage.root_cause, "locator-instability");
        assert!(!triage.evidence_frame_indexes.is_empty());
    }

    #[test]
    fn test_flake_vector_from_traces() {
        let e = QaEngine::new();
        // Force distinct timestamps + digest churn via direct frame inject.
        {
            let mut traces = e.traces.lock();
            traces.insert(
                "login".into(),
                vec![
                    TraceFrame {
                        index: 0,
                        step: "a".into(),
                        action: "click".into(),
                        dom_digest: "aaa".into(),
                        dom_snapshot: "<a/>".into(),
                        timestamp_ms: 1000,
                    },
                    TraceFrame {
                        index: 1,
                        step: "b".into(),
                        action: "click".into(),
                        dom_digest: "bbb".into(),
                        dom_snapshot: "<b/>".into(),
                        timestamp_ms: 1005, // race-y 5ms gap
                    },
                    TraceFrame {
                        index: 2,
                        step: "c".into(),
                        action: "click".into(),
                        dom_digest: "ccc".into(),
                        dom_snapshot: "<c/>".into(),
                        timestamp_ms: 1010,
                    },
                ],
            );
        }
        let frames = e.trace_frames("login");
        let derived = FlakeVector::from_trace_frames(&frames);
        assert!(derived.locator_churn > 0.5, "churn={:?}", derived);
        assert!(derived.race_score > 0.0);
        assert!(!derived.is_empty());

        let mut headers = HashMap::new();
        headers.insert("x-response-time".into(), "10ms".into());
        let mut headers2 = HashMap::new();
        headers2.insert("x-response-time".into(), "80ms".into());
        let network_traces = [headers, headers2].map(|headers| NetworkTrace {
            method: "GET".into(),
            url: "https://x/a".into(),
            headers,
            body: None,
            status: 200,
        });
        let net = FlakeVector::from_network_traces(&network_traces);
        assert!(net.network_jitter_ms > 0.0);

        let radar = e.flake_radar("login", &[true, false, true, false]);
        let triage = e.triage_flake("login", &radar, &derived, &frames);
        assert!(!triage.root_cause.is_empty());
        assert_ne!(triage.root_cause, "unknown");
    }

    #[test]
    fn test_quarantine_policy() {
        let tmp = std::env::temp_dir().join(format!("thor-qa-q-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("THOR_DATA_DIR", &tmp);

        let e = QaEngine::new();
        let d = e.evaluate_quarantine(&QuarantineInput {
            test: "flaky".into(),
            flake_rate: 0.4,
            consecutive_fails: 0,
            days_in_quarantine: 0.0,
            green_streak: 0,
            currently_quarantined: false,
        });
        assert_eq!(d.action, "quarantine");
        assert!(d.quarantine_yaml.contains("quarantine"));
        assert!(
            d.persist_ok,
            "evaluate_quarantine must persist yaml: {:?}",
            d.persist_err
        );
        assert!(d.persist_err.is_none());
        let path = QaEngine::quarantine_path();
        assert!(
            path.exists(),
            "expected yaml after evaluate at {}",
            path.display()
        );
        let yaml = QaEngine::load_quarantine_yaml().unwrap();
        assert!(yaml.contains("flaky"));
        let filter = e.ci_quarantine_filter();
        assert!(!filter.is_empty());
        assert!(filter.contains("--grep-invert"));
        assert!(filter.contains("flaky"));

        let empty_engine = QaEngine::new();
        let composed = empty_engine.export_ci_bundle_composed(
            &crate::dx::test_generator::RecordedSession::new("ci-empty", "https://example.test"),
            None,
        );
        assert!(!composed.playwright_ts.trim().is_empty());
        assert!(!composed.hermetic_ts.trim().is_empty());
        assert!(!composed.quarantine_filter.trim().is_empty());

        let readmit = e.evaluate_quarantine(&QuarantineInput {
            test: "flaky".into(),
            flake_rate: 0.0,
            consecutive_fails: 0,
            days_in_quarantine: 2.0,
            green_streak: 5,
            currently_quarantined: true,
        });
        assert_eq!(readmit.action, "readmit");
        assert!(
            readmit.persist_ok,
            "readmit persist: {:?}",
            readmit.persist_err
        );

        std::env::remove_var("THOR_DATA_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_contract_drift_gate() {
        let e = QaEngine::new();
        let baseline = vec![ContractInteraction {
            description: "get".into(),
            method: "GET".into(),
            path: "/u/1".into(),
            expected_status: 200,
            response_schema: None,
        }];
        let live = vec![ActualInteraction {
            method: "GET".into(),
            path: "/u/1".into(),
            status: 500,
            body: None,
        }];
        let r = e.contract_drift_gate(&baseline, &live);
        assert!(r.breaking);
        assert!(r.changes.iter().any(|c| c.kind == "status-changed"));
    }

    #[test]
    fn test_hermetic_fixture_pack() {
        let e = QaEngine::new();
        let mut identity = IdentityFixture {
            name: "admin".into(),
            role: "admin".into(),
            ..Default::default()
        };
        identity.cookies.insert("sid".into(), "abc".into());
        identity.tokens.insert("jwt".into(), "xyz".into());
        let pack = e.build_hermetic_fixture_pack(
            "admin-pack",
            identity,
            vec![HarMockEntry {
                method: "GET".into(),
                url: "/api/me".into(),
                status: 200,
                body: "{}".into(),
            }],
            vec![StorageSeed {
                origin: "https://app".into(),
                local_storage: HashMap::from([("theme".into(), "dark".into())]),
                ..Default::default()
            }],
        );
        assert!(!pack.pack_digest.is_empty());
        assert_eq!(pack.har_mocks.len(), 1);
        let pw = pack.to_playwright();
        assert!(pw.contains("route.fulfill"), "{pw}");
        assert!(pw.contains("storageState"), "{pw}");
        let state: serde_json::Value =
            serde_json::from_str(&pack.storage_state_json()).expect("storageState JSON");
        assert!(state["cookies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "sid"));
        assert!(state["origins"].as_array().unwrap().iter().any(|o| {
            o["origin"] == "https://app"
                && o["localStorage"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["name"] == "theme" && e["value"] == "dark")
        }));
    }

    #[test]
    fn test_a11y_test_gen() {
        let e = QaEngine::new();
        let spec = e.generate_a11y_spec(
            "checkout",
            &[A11yViolation {
                rule_id: "image-alt".into(),
                selector: "img.hero".into(),
                impact: "critical".into(),
                message: "Images must have alt text".into(),
            }],
        );
        assert!(spec.playwright_ts.contains("AxeBuilder"));
        assert!(spec.playwright_ts.contains("image-alt"));
        assert!(spec.playwright_ts.contains("@axe-core/playwright"));
        assert_eq!(spec.violation_count, 1);
    }

    #[test]
    fn test_compose_oracles_into_export() {
        let e = QaEngine::new();
        let session = RecordedSession::new("Flow", "https://x");
        let composed = e.export_with_oracles(
            &session,
            &OracleObservation {
                status: Some(200),
                dom_digest: Some("abc".into()),
                ..Default::default()
            },
        );
        assert!(composed.contains("Thor synthesized oracles"));
        assert!(composed.contains("toBe(200)"));
        assert!(composed.contains("toBe('abc')"));
    }

    #[test]
    fn test_snapshot_strategy_pack() {
        let e = QaEngine::new();
        let m = e.snapshot_strategy_pack("home", &[1, 2, 3, 4], "<div/>", "a11y", "{}");
        assert_eq!(m.layers.len(), 4);
        assert!(m.manifesto_json.contains("pixels"));
    }

    #[test]
    fn test_coverage_adjacency_map() {
        let e = QaEngine::new();
        let observed = vec![
            CoverageEdge {
                kind: "route".into(),
                from: "/".into(),
                to: "/cart".into(),
            },
            CoverageEdge {
                kind: "api".into(),
                from: "UI".into(),
                to: "GET /cart".into(),
            },
        ];
        let covered = vec![observed[0].clone()];
        let r = e.coverage_adjacency(&observed, &covered);
        assert_eq!(r.uncovered.len(), 1);
        assert!((r.coverage_ratio - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_parallel_shard_planner() {
        let e = QaEngine::new();
        let scenarios = vec![
            ShardScenario {
                name: "a".into(),
                duration_ms: 100.0,
                identity: "admin".into(),
            },
            ShardScenario {
                name: "b".into(),
                duration_ms: 200.0,
                identity: "user".into(),
            },
            ShardScenario {
                name: "c".into(),
                duration_ms: 50.0,
                identity: "admin".into(),
            },
        ];
        let plan = e.plan_parallel_shards(&scenarios, 2);
        assert_eq!(plan.shard_count, 2);
        assert!(plan.plan_json.contains("shard_count"));
        let total: usize = plan.shards.iter().map(|s| s.scenarios.len()).sum();
        assert_eq!(total, 3);
    }

    #[test]
    fn test_trace_property_miner() {
        let e = QaEngine::new();
        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), "Bearer x".into());
        let traces = vec![
            NetworkTrace {
                method: "POST".into(),
                url: "https://x/admin/items?page=1".into(),
                headers: headers.clone(),
                body: None,
                status: 200,
            },
            NetworkTrace {
                method: "POST".into(),
                url: "https://x/admin/items?page=1".into(),
                headers,
                body: None,
                status: 200,
            },
        ];
        let cases = e.mine_trace_properties(&traces);
        assert!(cases.iter().any(|c| c.kind == "idempotency"));
        assert!(cases.iter().any(|c| c.kind == "pagination"));
        assert!(cases.iter().any(|c| c.kind == "authz"));
    }

    #[test]
    fn test_runtime_self_heal() {
        let e = QaEngine::new();
        let signals = ElementSignals {
            test_id: Some("submit".into()),
            css: Some(".btn".into()),
            ..Default::default()
        };
        let live = vec![
            HealSignal {
                locator: "page.getByTestId('submit')".into(),
                strategy: "test-id".into(),
                successes: 0,
                failures: 5,
                last_error: "not found".into(),
            },
            HealSignal {
                locator: "page.locator('.btn')".into(),
                strategy: "css".into(),
                successes: 10,
                failures: 0,
                last_error: String::new(),
            },
        ];
        let patch = e.runtime_self_heal(&signals, &live);
        assert!(!patch.patch_json.is_empty());
        assert!(!patch.ranked.is_empty());
    }

    #[test]
    fn test_oracle_synthesizer() {
        let e = QaEngine::new();
        let mut obs = OracleObservation {
            dom_digest: Some("abc".into()),
            status: Some(200),
            vitals: Some(BrowserMetrics {
                lcp_ms: 1200.0,
                cls: 0.01,
                ..Default::default()
            }),
            ..Default::default()
        };
        obs.headers
            .insert("content-type".into(), "application/json".into());
        obs.storage.insert("token".into(), "t".into());
        let oracles = e.synthesize_oracles(&obs);
        assert!(oracles.iter().any(|o| o.kind == "dom"));
        assert!(oracles.iter().any(|o| o.kind == "status"));
        assert!(oracles.iter().any(|o| o.kind == "header"));
        assert!(oracles.iter().any(|o| o.kind == "vitals"));
        assert!(oracles.iter().any(|o| o.kind == "storage"));
    }

    #[test]
    fn test_mutation_hook_bridge() {
        let e = QaEngine::new();
        let m = e.mutation_hook_bridge(&[CoveredStep {
            id: "step-1".into(),
            file: "src/cart.rs".into(),
            mutation_targets: vec!["cart::add".into()],
        }]);
        assert_eq!(m.covered_steps, 1);
        assert_eq!(m.stryker["mutate"][0], "src/cart.rs");
        assert_eq!(m.cargo_mutants["examine"][0], "cart::add");
    }
}

#[cfg(test)]
mod quarantine_tests;
#[cfg(test)]
mod vector_tests;
