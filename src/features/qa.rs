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

impl FlakeVector {
    /// True when every signal is unset (hand-fed empty / default).
    pub fn is_empty(&self) -> bool {
        self.timing_cv == 0.0
            && self.locator_churn == 0.0
            && self.race_score == 0.0
            && self.network_jitter_ms == 0.0
            && self.animation_score == 0.0
    }

    /// Merge two vectors by taking the per-field maximum (strongest signal wins).
    pub fn merge(self, other: Self) -> Self {
        Self {
            timing_cv: self.timing_cv.max(other.timing_cv),
            locator_churn: self.locator_churn.max(other.locator_churn),
            race_score: self.race_score.max(other.race_score),
            network_jitter_ms: self.network_jitter_ms.max(other.network_jitter_ms),
            animation_score: self.animation_score.max(other.animation_score),
        }
    }

    /// Derive timing CV, locator churn, race, and animation scores from
    /// time-travel trace frames (no hand-fed vector required).
    pub fn from_trace_frames(frames: &[TraceFrame]) -> Self {
        if frames.len() < 2 {
            return Self::default();
        }
        let mut deltas = Vec::with_capacity(frames.len() - 1);
        let mut digest_churns = 0usize;
        let mut race_hits = 0usize;
        for w in frames.windows(2) {
            let dt = w[1].timestamp_ms.saturating_sub(w[0].timestamp_ms) as f64;
            deltas.push(dt);
            if w[0].dom_digest != w[1].dom_digest {
                digest_churns += 1;
            }
            // Sub-16ms adjacent frames suggest a timing race / animation frame fight.
            if dt > 0.0 && dt < 16.0 {
                race_hits += 1;
            }
        }
        let n = deltas.len() as f64;
        let mean = deltas.iter().sum::<f64>() / n;
        let variance = deltas
            .iter()
            .map(|d| {
                let x = d - mean;
                x * x
            })
            .sum::<f64>()
            / n;
        let std = variance.sqrt();
        let timing_cv = if mean > 0.0 {
            (std / mean).min(2.0)
        } else if std > 0.0 {
            1.0
        } else {
            0.0
        };
        let locator_churn = (digest_churns as f64 / n).clamp(0.0, 1.0);
        let race_score = (race_hits as f64 / n).clamp(0.0, 1.0);
        // Animation score: high digest churn with low mean inter-frame gap.
        let animation_score = if mean > 0.0 && mean < 50.0 {
            (locator_churn * (1.0 - (mean / 50.0))).clamp(0.0, 1.0)
        } else {
            (locator_churn * 0.25).clamp(0.0, 1.0)
        };
        Self {
            timing_cv: timing_cv.clamp(0.0, 1.0),
            locator_churn,
            race_score,
            network_jitter_ms: 0.0,
            animation_score,
        }
    }

    /// Derive network jitter from recorded traces — prefers numeric timing
    /// headers (`x-response-time`, `server-timing`), else status flip rate.
    pub fn from_network_traces(traces: &[NetworkTrace]) -> Self {
        if traces.is_empty() {
            return Self::default();
        }
        let mut timings = Vec::new();
        for t in traces {
            if let Some(ms) = extract_trace_timing_ms(t) {
                timings.push(ms);
            }
        }
        let network_jitter_ms = if timings.len() >= 2 {
            let mean = timings.iter().sum::<f64>() / timings.len() as f64;
            let var = timings
                .iter()
                .map(|d| {
                    let x = d - mean;
                    x * x
                })
                .sum::<f64>()
                / timings.len() as f64;
            var.sqrt()
        } else {
            // No timing headers — use status flip rate × 100ms as jitter proxy.
            let flips = traces
                .windows(2)
                .filter(|w| w[0].status != w[1].status)
                .count();
            let pairs = traces.len().saturating_sub(1).max(1);
            (flips as f64 / pairs as f64) * 100.0
        };
        Self {
            network_jitter_ms,
            ..Default::default()
        }
    }
}

fn extract_trace_timing_ms(t: &NetworkTrace) -> Option<f64> {
    for (k, v) in &t.headers {
        let key = k.to_ascii_lowercase();
        if key == "x-response-time" || key == "x-runtime" || key == "x-request-time" {
            let cleaned = v.trim_end_matches("ms").trim();
            if let Ok(ms) = cleaned.parse::<f64>() {
                return Some(ms);
            }
        }
        if key == "server-timing" {
            // e.g. "app;dur=42.5" — take first dur=
            if let Some(idx) = v.to_ascii_lowercase().find("dur=") {
                let rest = &v[idx + 4..];
                let num: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if let Ok(ms) = num.parse::<f64>() {
                    return Some(ms);
                }
            }
        }
    }
    None
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

impl HermeticFixturePack {
    /// Playwright `storageState`-shaped JSON (cookies + origins/localStorage).
    pub fn storage_state_json(&self) -> String {
        let cookies: Vec<serde_json::Value> = self
            .identity
            .cookies
            .iter()
            .map(|(k, v)| {
                serde_json::json!({
                    "name": k,
                    "value": v,
                    "domain": ".",
                    "path": "/",
                })
            })
            .collect();
        let mut origins: Vec<serde_json::Value> = Vec::new();
        for seed in &self.storage {
            let mut ls: Vec<serde_json::Value> = seed
                .local_storage
                .iter()
                .map(|(k, v)| serde_json::json!({ "name": k, "value": v }))
                .collect();
            for (k, v) in &self.identity.tokens {
                ls.push(serde_json::json!({ "name": k, "value": v }));
            }
            for (k, v) in &seed.session_storage {
                ls.push(serde_json::json!({ "name": format!("session:{k}"), "value": v }));
            }
            origins.push(serde_json::json!({
                "origin": seed.origin,
                "localStorage": ls,
            }));
        }
        if origins.is_empty() && !self.identity.tokens.is_empty() {
            origins.push(serde_json::json!({
                "origin": "",
                "localStorage": self.identity.tokens.iter()
                    .map(|(k, v)| serde_json::json!({ "name": k, "value": v }))
                    .collect::<Vec<_>>(),
            }));
        }
        serde_json::to_string_pretty(&serde_json::json!({
            "cookies": cookies,
            "origins": origins,
        }))
        .unwrap_or_else(|_| "{\"cookies\":[],\"origins\":[]}".into())
    }

    /// Emit a Playwright setup that installs `route.fulfill` HAR mocks and
    /// writes `storageState` JSON for hermetic runs.
    pub fn to_playwright(&self) -> String {
        let mut out = String::new();
        out.push_str("// Generated by Thor HermeticFixturePack\n");
        out.push_str("import { test, expect } from '@playwright/test';\n");
        out.push_str("import fs from 'fs';\n\n");
        out.push_str(&format!(
            "const storageState = {} as const;\n",
            self.storage_state_json()
        ));
        out.push_str(&format!(
            "fs.writeFileSync('storageState-{}.json', JSON.stringify(storageState, null, 2));\n\n",
            self.name.replace(|c: char| !c.is_alphanumeric(), "-")
        ));
        out.push_str(&format!(
            "test.describe('hermetic: {}', () => {{\n",
            self.name.replace('\'', "\\'")
        ));
        out.push_str("  test.beforeEach(async ({ page, context }) => {\n");
        out.push_str("    await context.addCookies(storageState.cookies as any);\n");
        for m in &self.har_mocks {
            let url = m.url.replace('\'', "\\'");
            let body = m.body.replace('\\', "\\\\").replace('\'', "\\'");
            out.push_str(&format!(
                "    await page.route('**{}', async (route) => {{\n",
                if url.starts_with("http") {
                    // full URL — use path portion for route glob
                    let path = url
                        .split("://")
                        .nth(1)
                        .and_then(|r| r.find('/').map(|i| &r[i..]))
                        .unwrap_or(url.as_str());
                    path.replace('\'', "\\'")
                } else {
                    url
                }
            ));
            out.push_str(&format!(
                "      if (route.request().method() !== '{}') {{ await route.fallback(); return; }}\n",
                m.method.to_uppercase()
            ));
            out.push_str(&format!(
                "      await route.fulfill({{ status: {}, body: '{}' }});\n",
                m.status, body
            ));
            out.push_str("    });\n");
        }
        for seed in &self.storage {
            if seed.local_storage.is_empty() && seed.session_storage.is_empty() {
                continue;
            }
            out.push_str("    await page.addInitScript((seed) => {\n");
            out.push_str("      for (const [k, v] of Object.entries(seed.localStorage || {})) localStorage.setItem(k, v);\n");
            out.push_str("      for (const [k, v] of Object.entries(seed.sessionStorage || {})) sessionStorage.setItem(k, v);\n");
            out.push_str(&format!(
                "    }}, {});",
                serde_json::json!({
                    "localStorage": seed.local_storage,
                    "sessionStorage": seed.session_storage,
                })
            ));
            out.push('\n');
        }
        out.push_str("  });\n\n");
        out.push_str(&format!(
            "  test('loads with {} fixture', async ({{ page }}) => {{\n",
            self.identity.role.replace('\'', "\\'")
        ));
        out.push_str("    // consumers navigate + assert against fulfilled routes\n");
        out.push_str("    expect(storageState).toBeTruthy();\n");
        out.push_str("  });\n");
        out.push_str("});\n");
        out
    }
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

    /// Load quarantine.yaml into the in-memory map (best-effort).
    fn hydrate_quarantine(&self) {
        let Ok(yaml) = Self::load_quarantine_yaml() else {
            return;
        };
        self.ingest_quarantine_yaml(&yaml);
    }

    /// Parse a quarantine.yaml body into the in-memory map.
    pub fn ingest_quarantine_yaml(&self, yaml: &str) {
        let mut pending_test: Option<String> = None;
        let mut pending_action: Option<String> = None;
        let mut pending_reason = String::new();
        let mut map = self.quarantine.lock();
        for line in yaml.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("- test:") {
                if let (Some(test), Some(action)) = (pending_test.take(), pending_action.take()) {
                    map.insert(
                        test.clone(),
                        QuarantineDecision {
                            test: test.clone(),
                            action: action.clone(),
                            reason: pending_reason.clone(),
                            quarantine_yaml: format!(
                                "quarantine:\n  - test: \"{}\"\n    action: {action}\n    reason: \"{}\"\n",
                                test.replace('"', "\\\""),
                                pending_reason.replace('"', "\\\"")
                            ),
                            persist_ok: true,
                            persist_err: None,
                        },
                    );
                }
                pending_reason.clear();
                pending_action = None;
                pending_test = Some(
                    rest.trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                );
            } else if let Some(rest) = trimmed.strip_prefix("action:") {
                pending_action = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed.strip_prefix("reason:") {
                pending_reason = rest
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
            }
        }
        if let (Some(test), Some(action)) = (pending_test, pending_action) {
            map.insert(
                test.clone(),
                QuarantineDecision {
                    test: test.clone(),
                    action: action.clone(),
                    reason: pending_reason.clone(),
                    quarantine_yaml: format!(
                        "quarantine:\n  - test: \"{}\"\n    action: {action}\n    reason: \"{}\"\n",
                        test.replace('"', "\\\""),
                        pending_reason.replace('"', "\\\"")
                    ),
                    persist_ok: true,
                    persist_err: None,
                },
            );
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

    /// Replay a load scenario `n` times through the protocol runner, treating a
    /// clean run (no budget violation / transport error) as a pass, and classify
    /// the result with the flake radar. This exercises the *active* half of the
    /// flake radar rather than accepting a caller-supplied result vector.
    pub fn flake_radar_replay(&self, scenario: &ThorScenario, n: usize) -> FlakeReplayReport {
        let runner = ProtocolRunner::new();
        let mut results = Vec::with_capacity(n);
        let mut errors = Vec::new();
        for _ in 0..n {
            match runner.run_blocking(scenario) {
                Ok(_) => results.push(true),
                Err(e) => {
                    results.push(false);
                    errors.push(e);
                }
            }
        }
        let radar = self.flake_radar(&scenario.name, &results);
        FlakeReplayReport {
            scenario: scenario.name.clone(),
            requested_runs: n,
            results,
            errors,
            radar,
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
            masked_pixels: 0,
        }
    }

    /// Visual regression with masked regions: pixels covered by any
    /// [`MaskRegion`] (e.g. resolved from a dynamic selector's bounding box) are
    /// excluded from the diff so volatile content — timestamps, avatars, ads —
    /// does not fail the comparison. `width` is the row stride of the grayscale
    /// buffers.
    pub fn visual_regression_masked(
        &self,
        baseline: &[u8],
        candidate: &[u8],
        width: usize,
        threshold: f64,
        mask_regions: &[MaskRegion],
    ) -> VisualDiff {
        if width == 0 || mask_regions.is_empty() {
            return self.visual_regression(baseline, candidate, threshold);
        }
        let mut masked: Vec<bool> = vec![false; baseline.len().min(candidate.len())];
        for region in mask_regions {
            for row in region.y..region.y.saturating_add(region.height) {
                let row_start = row * width;
                for col in region.x..region.x.saturating_add(region.width) {
                    let idx = row_start + col;
                    if idx < masked.len() {
                        masked[idx] = true;
                    }
                }
            }
        }
        let masked_pixels = masked.iter().filter(|m| **m).count();
        let compared = masked.len().saturating_sub(masked_pixels).max(1);
        let diff_pixels = baseline
            .iter()
            .zip(candidate.iter())
            .enumerate()
            .filter(|(i, (a, b))| !masked[*i] && a.abs_diff(**b) > 8)
            .count();
        let size_penalty = baseline.len().abs_diff(candidate.len());
        let pixel_diff_ratio = (diff_pixels + size_penalty) as f64 / compared as f64;
        let perceptual_distance = (dhash(baseline) ^ dhash(candidate)).count_ones();
        VisualDiff {
            pixel_diff_ratio,
            perceptual_distance,
            passed: pixel_diff_ratio <= threshold,
            masked_pixels,
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

    /// Emit a CI bundle manifest: workflow + composed hermetic/oracles/a11y/quarantine.
    pub fn export_ci_bundle_manifest(&self, session: &RecordedSession) -> CiBundleManifest {
        self.export_ci_bundle_composed(session, None)
    }

    /// Unify CI promote: hermetic fixture + oracles + a11y scaffold + quarantine filter.
    ///
    /// Always returns non-empty `playwright_ts`, `hermetic_ts`, and
    /// `quarantine_filter` (stub filter when the quarantine map is empty).
    pub fn export_ci_bundle_composed(
        &self,
        session: &RecordedSession,
        observation: Option<&OracleObservation>,
    ) -> CiBundleManifest {
        let safe_name = session
            .name
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "-");
        let mut playwright_ts = match observation {
            Some(obs) => self.export_with_oracles(session, obs),
            None => {
                let empty = OracleObservation {
                    dom_digest: None,
                    status: Some(200),
                    headers: HashMap::new(),
                    vitals: None,
                    storage: HashMap::new(),
                };
                self.export_with_oracles(session, &empty)
            }
        };
        if playwright_ts.trim().is_empty() {
            playwright_ts =
                TestGenerator::promote_to_test(session, ExportFormat::PlaywrightTs);
        }
        if playwright_ts.trim().is_empty() {
            playwright_ts = String::from(
                "// Thor CI bundle stub\nimport { test } from '@playwright/test';\ntest('stub', async () => {});\n",
            );
        }
        let identity = IdentityFixture {
            name: format!("{}-ci", session.name),
            role: "ci".into(),
            ..Default::default()
        };
        let pack = self.build_hermetic_fixture_pack(
            &format!("{}-pack", safe_name),
            identity,
            Vec::new(),
            Vec::new(),
        );
        let mut hermetic_ts = pack.to_playwright();
        if hermetic_ts.trim().is_empty() {
            hermetic_ts = String::from(
                "// Thor hermetic fixture stub\nimport { test } from '@playwright/test';\ntest.describe('hermetic stub', () => {});\n",
            );
        }
        let a11y_scaffold =
            TestGenerator::a11y_spec_scaffold(&session.name, &[]);
        let mut quarantine_filter = self.ci_quarantine_filter();
        if quarantine_filter.trim().is_empty() {
            quarantine_filter = TestGenerator::ci_quarantine_filter(&[]);
        }
        CiBundleManifest {
            workflow: TestGenerator::promote_to_test(session, ExportFormat::CiBundle),
            spec_path: format!("tests/{safe_name}.spec.ts"),
            junit_path: format!("test-results/{safe_name}.junit.xml"),
            har_path: format!("test-results/{safe_name}.har"),
            playwright_ts,
            hermetic_ts,
            a11y_scaffold,
            quarantine_filter,
        }
    }

    // 10. Time-Travel Trace -------------------------------------------------
    /// Record a trace frame, storing both a change-detection digest and the full
    /// DOM markup so a frame can be replayed, not merely diffed.
    pub fn record_trace_frame(&self, test: &str, step: &str, action: &str, dom: &str) -> usize {
        let mut traces = self.traces.lock();
        let frames = traces.entry(test.to_string()).or_default();
        let index = frames.len();
        frames.push(TraceFrame {
            index,
            step: step.to_string(),
            action: action.to_string(),
            dom_digest: format!("{:016x}", dhash(dom.as_bytes())),
            dom_snapshot: dom.to_string(),
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

    /// Export recorded frames to a Playwright-style on-disk trace directory
    /// (`trace.meta.json` + per-frame HTML snapshots) under Thor data dir.
    pub fn export_playwright_trace(&self, test: &str) -> Result<PlaywrightTraceExport, String> {
        let frames = self.trace_frames(test);
        if frames.is_empty() {
            return Err(format!(
                "no time-travel frames for test '{test}' — record with from_active_tab first"
            ));
        }
        let safe: String = test
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let dir = crate::paths::thor_data_dir()
            .join("traces")
            .join(&safe);
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        let mut frame_paths = Vec::new();
        for frame in &frames {
            let name = format!("frame-{:04}.html", frame.index);
            let path = dir.join(&name);
            std::fs::write(&path, &frame.dom_snapshot)
                .map_err(|e| format!("write {}: {e}", path.display()))?;
            frame_paths.push(name);
        }
        let meta = serde_json::json!({
            "version": 1,
            "test": test,
            "frame_count": frames.len(),
            "frames": frames.iter().map(|f| serde_json::json!({
                "index": f.index,
                "step": f.step,
                "action": f.action,
                "dom_digest": f.dom_digest,
                "timestamp_ms": f.timestamp_ms,
                "snapshot": format!("frame-{:04}.html", f.index),
            })).collect::<Vec<_>>(),
        });
        let meta_path = dir.join("trace.meta.json");
        std::fs::write(
            &meta_path,
            serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".into()),
        )
        .map_err(|e| format!("write meta: {e}"))?;
        Ok(PlaywrightTraceExport {
            test: test.to_string(),
            dir: dir.display().to_string(),
            frame_count: frames.len(),
            frame_paths,
            meta_path: meta_path.display().to_string(),
            open_hint: format!(
                "open {} — Thor DOM time-travel export (HTML snapshots + meta)",
                dir.display()
            ),
        })
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

    /// Build a CDP emulation script bundle that pins the browser's
    /// non-deterministic inputs (virtual time origin, RNG, timezone, locale,
    /// reduced motion) so repeated runs are reproducible. The returned commands
    /// are ready to feed to `Emulation.*` / `Page.addScriptToEvaluateOnNewDocument`.
    pub fn determinism_controls(&self, seed: u64, epoch_ms: u64) -> DeterminismControls {
        let init_script = concat!(
            "(() => {\n",
            "  // Thor determinism harness: pin RNG + clock.\n",
            "  let __seed = __SEED__ >>> 0;\n",
            "  Math.random = () => { __seed = (1103515245 * __seed + 12345) & 0x7fffffff; return __seed / 0x7fffffff; };\n",
            "  const __base = __EPOCH__;\n",
            "  const __RealDate = Date;\n",
            "  Date.now = () => __base;\n",
            "})();"
        )
        .replace("__SEED__", &seed.to_string())
        .replace("__EPOCH__", &epoch_ms.to_string());
        let cdp_commands = vec![
            serde_json::json!({
                "method": "Emulation.setTimezoneOverride",
                "params": { "timezoneId": "UTC" }
            }),
            serde_json::json!({
                "method": "Emulation.setLocaleOverride",
                "params": { "locale": "en-US" }
            }),
            serde_json::json!({
                "method": "Emulation.setEmulatedMedia",
                "params": { "features": [{ "name": "prefers-reduced-motion", "value": "reduce" }] }
            }),
            serde_json::json!({
                "method": "Emulation.setVirtualTimePolicy",
                "params": { "policy": "pause", "initialVirtualTime": (epoch_ms as f64) / 1000.0 }
            }),
            serde_json::json!({
                "method": "Page.addScriptToEvaluateOnNewDocument",
                "params": { "source": init_script }
            }),
        ];
        DeterminismControls {
            cdp_commands,
            init_script,
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

    // ---- Wave 2 -----------------------------------------------------------

    /// Attribute flake root-cause from a signal vector plus optional time-travel
    /// frames (DOM digest churn around failing steps).
    pub fn triage_flake(
        &self,
        test: &str,
        radar: &FlakeReport,
        vectors: &FlakeVector,
        frames: &[TraceFrame],
    ) -> FlakeTriage {
        let mut candidates = vec![
            ("timing-race", vectors.timing_cv + vectors.race_score),
            ("locator-instability", vectors.locator_churn),
            ("network-jitter", (vectors.network_jitter_ms / 250.0).min(1.0)),
            ("animation-layout", vectors.animation_score),
        ];
        if radar.classification == "consistently-failing" {
            candidates.push(("deterministic-failure", 0.95));
        } else if radar.classification == "stable-pass" {
            candidates.push(("no-flake", 1.0));
        }
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let (root_cause, raw) = candidates
            .first()
            .copied()
            .unwrap_or(("unknown", 0.0));
        let confidence = raw.clamp(0.0, 1.0);

        let mut evidence = Vec::new();
        if frames.len() >= 2 {
            for w in frames.windows(2) {
                if w[0].dom_digest != w[1].dom_digest {
                    evidence.push(w[1].index);
                }
            }
        }
        if evidence.is_empty() && !frames.is_empty() {
            evidence.push(frames.last().map(|f| f.index).unwrap_or(0));
        }

        let recommended_action = match root_cause {
            "locator-instability" => "re-rank locators / enable runtime-self-heal",
            "timing-race" => "replace fixed waits with web-first assertions",
            "network-jitter" => "hermetic fixture pack / HAR mocks",
            "animation-layout" => "mask volatile regions in visual regression",
            "deterministic-failure" => "quarantine until root fix lands",
            "no-flake" => "keep green; no action",
            _ => "collect more flake vectors",
        };

        FlakeTriage {
            test: test.to_string(),
            root_cause: root_cause.to_string(),
            confidence,
            evidence_frame_indexes: evidence,
            recommended_action: recommended_action.to_string(),
        }
    }

    /// Decide quarantine / skip / expire / re-admit for a flaky test.
    /// Persists the in-memory quarantine map to `thor_data_dir()/quarantine.yaml`.
    ///
    /// Persist outcome is never swallowed: `persist_ok` / `persist_err` report
    /// whether `quarantine.yaml` was written.
    pub fn evaluate_quarantine(&self, input: &QuarantineInput) -> QuarantineDecision {
        let action = if input.currently_quarantined {
            if input.green_streak >= 5 {
                "readmit"
            } else if input.days_in_quarantine >= 14.0 {
                "expire"
            } else if input.flake_rate >= 0.5 {
                "skip"
            } else {
                "keep"
            }
        } else if input.flake_rate >= 0.3 || input.consecutive_fails >= 3 {
            "quarantine"
        } else {
            "keep"
        };
        let reason = match action {
            "quarantine" => format!(
                "flake_rate={:.2} consecutive_fails={}",
                input.flake_rate, input.consecutive_fails
            ),
            "skip" => "still highly flaky while quarantined".into(),
            "expire" => format!(
                "quarantine aged out after {:.1} days",
                input.days_in_quarantine
            ),
            "readmit" => format!("green streak of {}", input.green_streak),
            _ => "within policy thresholds".into(),
        };
        let yaml = format!(
            "quarantine:\n  - test: \"{}\"\n    action: {action}\n    reason: \"{}\"\n",
            input.test.replace('"', "\\\""),
            reason.replace('"', "\\\"")
        );
        let mut decision = QuarantineDecision {
            test: input.test.clone(),
            action: action.to_string(),
            reason,
            quarantine_yaml: yaml,
            persist_ok: false,
            persist_err: None,
        };
        self.quarantine
            .lock()
            .insert(input.test.clone(), decision.clone());
        match self.persist_quarantine() {
            Ok(_) => {
                decision.persist_ok = true;
                decision.persist_err = None;
            }
            Err(e) => {
                decision.persist_ok = false;
                decision.persist_err = Some(e);
            }
        }
        // Keep map entry in sync with persist outcome.
        self.quarantine
            .lock()
            .insert(input.test.clone(), decision.clone());
        decision
    }

    pub fn quarantine_status(&self, test: &str) -> Option<QuarantineDecision> {
        self.quarantine.lock().get(test).cloned()
    }

    /// Path to the on-disk quarantine policy (`thor_data_dir()/quarantine.yaml`).
    pub fn quarantine_path() -> std::path::PathBuf {
        crate::paths::thor_quarantine_path()
    }

    /// Write the in-memory quarantine map as YAML under `thor_quarantine_path()`.
    ///
    /// Merges with any existing on-disk entries so a process-wide engine that only
    /// knows about a subset of tests cannot clobber unrelated quarantine rows
    /// (important when tests share `THOR_DATA_DIR`).
    pub fn persist_quarantine(&self) -> Result<std::path::PathBuf, String> {
        let path = Self::quarantine_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create quarantine dir: {e}"))?;
        }
        let mut entries: std::collections::BTreeMap<String, (String, String)> =
            std::collections::BTreeMap::new();
        if let Ok(existing) = Self::load_quarantine_yaml() {
            // Seed from disk so we only patch known tests.
            let mut pending_test: Option<String> = None;
            let mut pending_action: Option<String> = None;
            let mut pending_reason = String::new();
            let flush = |entries: &mut std::collections::BTreeMap<String, (String, String)>,
                         test: Option<String>,
                         action: Option<String>,
                         reason: &str| {
                if let (Some(t), Some(a)) = (test, action) {
                    if matches!(a.as_str(), "quarantine" | "skip" | "keep") {
                        entries.insert(t, (a, reason.to_string()));
                    }
                }
            };
            for line in existing.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("- test:") {
                    flush(
                        &mut entries,
                        pending_test.take(),
                        pending_action.take(),
                        &pending_reason,
                    );
                    pending_reason.clear();
                    pending_test = Some(
                        rest.trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string(),
                    );
                } else if let Some(rest) = trimmed.strip_prefix("action:") {
                    pending_action = Some(rest.trim().to_string());
                } else if let Some(rest) = trimmed.strip_prefix("reason:") {
                    pending_reason = rest
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                }
            }
            flush(
                &mut entries,
                pending_test,
                pending_action,
                &pending_reason,
            );
        }
        {
            let map = self.quarantine.lock();
            for d in map.values() {
                match d.action.as_str() {
                    "quarantine" | "skip" | "keep" => {
                        entries.insert(d.test.clone(), (d.action.clone(), d.reason.clone()));
                    }
                    "expire" | "readmit" => {
                        entries.remove(&d.test);
                    }
                    _ => {}
                }
            }
        }
        let yaml = if entries.is_empty() {
            String::from("quarantine: []\n")
        } else {
            let list: Vec<(String, String, String)> = entries
                .into_iter()
                .map(|(test, (action, reason))| (test, action, reason))
                .collect();
            TestGenerator::quarantine_yaml(&list)
        };
        std::fs::write(&path, &yaml).map_err(|e| format!("write quarantine.yaml: {e}"))?;
        Ok(path)
    }

    /// Load quarantine.yaml from disk (best-effort; empty if missing).
    pub fn load_quarantine_yaml() -> Result<String, String> {
        let path = Self::quarantine_path();
        if !path.exists() {
            return Ok(String::from("quarantine: []\n"));
        }
        std::fs::read_to_string(&path).map_err(|e| format!("read quarantine.yaml: {e}"))
    }

    /// CI filter snippet for currently quarantined / skipped tests.
    /// Never empty — falls back to the stub from [`TestGenerator::ci_quarantine_filter`].
    pub fn ci_quarantine_filter(&self) -> String {
        let map = self.quarantine.lock();
        let titles: Vec<String> = map
            .values()
            .filter(|d| matches!(d.action.as_str(), "quarantine" | "skip"))
            .map(|d| d.test.clone())
            .collect();
        let filter = TestGenerator::ci_quarantine_filter(&titles);
        if filter.trim().is_empty() {
            TestGenerator::ci_quarantine_filter(&[])
        } else {
            filter
        }
    }

    /// Diff a baseline contract against live traffic / OpenAPI-shaped actuals and
    /// report breaking changes (removed paths, status shifts, schema breaks).
    pub fn contract_drift_gate(
        &self,
        baseline: &[ContractInteraction],
        live: &[ActualInteraction],
    ) -> ContractDriftReport {
        let mut changes = Vec::new();
        for expected in baseline {
            let matched = live.iter().find(|a| {
                a.method.eq_ignore_ascii_case(&expected.method) && a.path == expected.path
            });
            match matched {
                None => changes.push(ContractDriftChange {
                    path: format!("{} {}", expected.method, expected.path),
                    kind: "removed".into(),
                    detail: "baseline interaction missing from live traffic".into(),
                    breaking: true,
                }),
                Some(actual) => {
                    if actual.status != expected.expected_status {
                        changes.push(ContractDriftChange {
                            path: format!("{} {}", expected.method, expected.path),
                            kind: "status-changed".into(),
                            detail: format!(
                                "{} → {}",
                                expected.expected_status, actual.status
                            ),
                            breaking: true,
                        });
                    }
                    if let (Some(schema), Some(body)) = (&expected.response_schema, &actual.body) {
                        let errs = crate::features::backend::BackendEngine::new()
                            .validate_contract(body, schema);
                        for e in errs {
                            changes.push(ContractDriftChange {
                                path: format!("{} {}", expected.method, expected.path),
                                kind: "schema-breaking".into(),
                                detail: e,
                                breaking: true,
                            });
                        }
                    }
                }
            }
        }
        for actual in live {
            let known = baseline.iter().any(|b| {
                b.method.eq_ignore_ascii_case(&actual.method) && b.path == actual.path
            });
            if !known {
                changes.push(ContractDriftChange {
                    path: format!("{} {}", actual.method, actual.path),
                    kind: "added".into(),
                    detail: "live path not in baseline contract".into(),
                    breaking: false,
                });
            }
        }
        ContractDriftReport {
            breaking: changes.iter().any(|c| c.breaking),
            changes,
            baseline_count: baseline.len(),
            live_count: live.len(),
        }
    }

    /// Build a hermetic fixture pack: identity + HAR mocks + storage seeds.
    pub fn build_hermetic_fixture_pack(
        &self,
        name: &str,
        identity: IdentityFixture,
        har_mocks: Vec<HarMockEntry>,
        storage: Vec<StorageSeed>,
    ) -> HermeticFixturePack {
        let mut digest_src = format!("{}|{}|{}", name, identity.name, identity.role);
        for m in &har_mocks {
            digest_src.push_str(&format!("|{}:{}:{}", m.method, m.url, m.status));
        }
        for s in &storage {
            digest_src.push_str(&format!("|{}", s.origin));
            for (k, v) in &s.local_storage {
                digest_src.push_str(&format!("|ls:{k}={v}"));
            }
        }
        HermeticFixturePack {
            name: name.to_string(),
            identity,
            har_mocks,
            storage,
            pack_digest: format!("{:016x}", dhash(digest_src.as_bytes())),
        }
    }

    /// Promote a11y violations into a Playwright + axe-style assertion spec.
    /// Composes [`TestGenerator::a11y_spec_scaffold`] with per-violation asserts.
    pub fn generate_a11y_spec(&self, page_name: &str, violations: &[A11yViolation]) -> A11ySpec {
        let rule_ids: Vec<String> = violations.iter().map(|v| v.rule_id.clone()).collect();
        let mut out = TestGenerator::a11y_spec_scaffold(page_name, &rule_ids);
        let mut assertions = Vec::new();
        // Insert per-violation locator asserts before the closing `});`.
        if let Some(idx) = out.rfind("});") {
            let mut block = String::new();
            for v in violations {
                let assert_line = format!(
                    "expect(page.locator('{}')).not.toHaveAttribute('aria-invalid', 'true'); // {}",
                    v.selector.replace('\'', "\\'"),
                    v.rule_id
                );
                assertions.push(format!("{}: {}", v.rule_id, v.message));
                block.push_str(&format!(
                    "  // {} ({}) — {}\n  {}\n",
                    v.rule_id,
                    v.impact,
                    v.message.replace('\n', " "),
                    assert_line
                ));
            }
            out.insert_str(idx, &block);
        } else {
            for v in violations {
                assertions.push(format!("{}: {}", v.rule_id, v.message));
            }
        }
        A11ySpec {
            playwright_ts: out,
            axe_assertions: assertions,
            violation_count: violations.len(),
        }
    }

    /// Compose deflaked Playwright export with synthesized oracle assertions.
    pub fn export_with_oracles(
        &self,
        session: &RecordedSession,
        obs: &OracleObservation,
    ) -> String {
        let base = self.export_deflaked(session);
        let oracles = self.synthesize_oracles(obs);
        let lines: Vec<String> = oracles.into_iter().map(|o| o.assertion).collect();
        TestGenerator::append_oracles(&base, &lines)
    }

    /// Multi-layer snapshot manifesto (pixels / DOM / a11y / body hashes).
    pub fn snapshot_strategy_pack(
        &self,
        name: &str,
        pixels: &[u8],
        dom: &str,
        a11y_digest: &str,
        body: &str,
    ) -> SnapshotManifest {
        let layers = vec![
            SnapshotLayer {
                name: "pixels".into(),
                strategy: "grayscale-dhash".into(),
                hash: format!("{:016x}", dhash(pixels)),
            },
            SnapshotLayer {
                name: "dom".into(),
                strategy: "dom-digest".into(),
                hash: format!("{:016x}", dhash(dom.as_bytes())),
            },
            SnapshotLayer {
                name: "a11y".into(),
                strategy: "a11y-tree-hash".into(),
                hash: format!("{:016x}", dhash(a11y_digest.as_bytes())),
            },
            SnapshotLayer {
                name: "body".into(),
                strategy: "response-body-hash".into(),
                hash: format!("{:016x}", dhash(body.as_bytes())),
            },
        ];
        let manifesto = serde_json::json!({
            "name": name,
            "layers": layers,
        });
        SnapshotManifest {
            name: name.to_string(),
            layers,
            manifesto_json: serde_json::to_string_pretty(&manifesto).unwrap_or_default(),
        }
    }

    /// Diff observed session edges against covered edges → untested adjacency.
    pub fn coverage_adjacency(
        &self,
        observed: &[CoverageEdge],
        covered: &[CoverageEdge],
    ) -> CoverageAdjacencyReport {
        let covered_set: std::collections::HashSet<_> = covered.iter().cloned().collect();
        let uncovered: Vec<_> = observed
            .iter()
            .filter(|e| !covered_set.contains(*e))
            .cloned()
            .collect();
        let observed_n = observed.len();
        let covered_n = observed_n.saturating_sub(uncovered.len());
        CoverageAdjacencyReport {
            observed: observed_n,
            covered: covered_n,
            coverage_ratio: if observed_n == 0 {
                1.0
            } else {
                covered_n as f64 / observed_n as f64
            },
            uncovered,
        }
    }

    /// Balance scenarios into shards by duration, keeping identity affinity.
    pub fn plan_parallel_shards(
        &self,
        scenarios: &[ShardScenario],
        shard_count: usize,
    ) -> ShardPlan {
        let n = shard_count.max(1);
        let mut shards: Vec<ShardBucket> = (0..n)
            .map(|i| ShardBucket {
                shard_id: i,
                scenarios: Vec::new(),
                total_ms: 0.0,
                identities: Vec::new(),
            })
            .collect();

        // Group by identity so affinity stays within a shard when possible.
        let mut by_identity: HashMap<String, Vec<&ShardScenario>> = HashMap::new();
        for s in scenarios {
            by_identity
                .entry(s.identity.clone())
                .or_default()
                .push(s);
        }
        let mut groups: Vec<_> = by_identity.into_iter().collect();
        groups.sort_by(|a, b| {
            let sa: f64 = a.1.iter().map(|s| s.duration_ms).sum();
            let sb: f64 = b.1.iter().map(|s| s.duration_ms).sum();
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });

        for (identity, group) in groups {
            let mut target = 0usize;
            let mut best = f64::MAX;
            for (i, shard) in shards.iter().enumerate() {
                if shard.total_ms < best {
                    best = shard.total_ms;
                    target = i;
                }
            }
            if !identity.is_empty() {
                shards[target].identities.push(identity);
            }
            for s in group {
                shards[target].scenarios.push(s.name.clone());
                shards[target].total_ms += s.duration_ms;
            }
        }

        let plan_json = serde_json::to_string_pretty(&serde_json::json!({
            "shard_count": n,
            "shards": shards,
        }))
        .unwrap_or_else(|_| "{}".into());

        ShardPlan {
            shard_count: n,
            shards,
            plan_json,
        }
    }

    /// Infer property / idempotency / pagination / authz generative cases from
    /// recorded network traces.
    pub fn mine_trace_properties(&self, traces: &[NetworkTrace]) -> Vec<PropertyCase> {
        let mut cases = Vec::new();
        let mut by_key: HashMap<(String, String), Vec<&NetworkTrace>> = HashMap::new();
        for t in traces {
            let path = t.url.clone();
            by_key
                .entry((t.method.to_uppercase(), path))
                .or_default()
                .push(t);
        }
        for ((method, url), group) in &by_key {
            if method == "POST" || method == "PUT" || method == "PATCH" {
                cases.push(PropertyCase {
                    name: format!("idempotency:{method}:{url}"),
                    kind: "idempotency".into(),
                    description: format!(
                        "Replay {method} {url} with Idempotency-Key; expect stable status"
                    ),
                    seed_input: serde_json::json!({
                        "method": method,
                        "url": url,
                        "sample_status": group.first().map(|t| t.status),
                    }),
                });
            }
            let lower = url.to_lowercase();
            if lower.contains("page=")
                || lower.contains("cursor=")
                || lower.contains("offset=")
                || lower.contains("/next")
            {
                cases.push(PropertyCase {
                    name: format!("pagination:{url}"),
                    kind: "pagination".into(),
                    description: format!("Walk pagination params on {url}"),
                    seed_input: serde_json::json!({ "url": url, "method": method }),
                });
            }
            if lower.contains("/admin")
                || lower.contains("/internal")
                || group.iter().any(|t| {
                    t.headers.keys().any(|k| {
                        k.eq_ignore_ascii_case("authorization")
                            || k.eq_ignore_ascii_case("x-api-key")
                    })
                })
            {
                cases.push(PropertyCase {
                    name: format!("authz:{method}:{url}"),
                    kind: "authz".into(),
                    description: format!("Probe {method} {url} across identities"),
                    seed_input: serde_json::json!({ "url": url, "method": method }),
                });
            }
            if group.len() >= 2 {
                let statuses: Vec<_> = group.iter().map(|t| t.status).collect();
                cases.push(PropertyCase {
                    name: format!("property:status-stability:{url}"),
                    kind: "property".into(),
                    description: format!("Status multiset across repeats: {statuses:?}"),
                    seed_input: serde_json::json!({ "statuses": statuses }),
                });
            }
        }
        cases
    }

    /// Re-rank locators from live heal signals and emit a heal-patch artifact.
    pub fn runtime_self_heal(
        &self,
        signals: &ElementSignals,
        live: &[HealSignal],
    ) -> HealPatch {
        let mut ranked = self.rank_locators(signals);
        for loc in &mut ranked {
            if let Some(sig) = live.iter().find(|s| s.strategy == loc.strategy) {
                let total = sig.successes + sig.failures;
                let success_rate = if total == 0 {
                    0.5
                } else {
                    sig.successes as f64 / total as f64
                };
                let boost = (success_rate * 40.0) as u32;
                let penalty = if sig.failures > sig.successes { 25 } else { 0 };
                loc.resilience_score = loc.resilience_score.saturating_add(boost).saturating_sub(penalty);
            }
        }
        ranked.sort_by(|a, b| b.resilience_score.cmp(&a.resilience_score));
        let preferred = ranked.first().cloned().unwrap_or(RankedLocator {
            expression: signals
                .css
                .clone()
                .map(|c| format!("page.locator('{c}')"))
                .unwrap_or_else(|| "page.locator('body')".into()),
            strategy: "fallback".into(),
            resilience_score: 1,
        });
        let patch_json = serde_json::to_string_pretty(&serde_json::json!({
            "preferred": preferred,
            "ranked": ranked,
            "generated_ms": now_ms(),
        }))
        .unwrap_or_else(|_| "{}".into());
        HealPatch {
            preferred,
            ranked,
            patch_json,
        }
    }

    /// Synthesize oracles from DOM / status / headers / vitals / storage
    /// postconditions observed after a step.
    pub fn synthesize_oracles(&self, obs: &OracleObservation) -> Vec<OracleSpec> {
        let mut out = Vec::new();
        if let Some(dom) = &obs.dom_digest {
            out.push(OracleSpec {
                kind: "dom".into(),
                assertion: format!("expect(domDigest).toBe('{dom}')"),
                source: "dom_digest".into(),
            });
        }
        if let Some(status) = obs.status {
            out.push(OracleSpec {
                kind: "status".into(),
                assertion: format!("expect(response.status()).toBe({status})"),
                source: "status".into(),
            });
        }
        for (k, v) in &obs.headers {
            out.push(OracleSpec {
                kind: "header".into(),
                assertion: format!(
                    "expect(response.headers()['{}']).toBe('{}')",
                    k.to_lowercase(),
                    v.replace('\'', "\\'")
                ),
                source: format!("header:{k}"),
            });
        }
        if let Some(v) = &obs.vitals {
            out.push(OracleSpec {
                kind: "vitals".into(),
                assertion: format!(
                    "expect(vitals.lcp_ms).toBeLessThanOrEqual({}); expect(vitals.cls).toBeLessThanOrEqual({})",
                    v.lcp_ms.max(1.0),
                    (v.cls + 0.01).max(0.01)
                ),
                source: "vitals".into(),
            });
        }
        for (k, v) in &obs.storage {
            out.push(OracleSpec {
                kind: "storage".into(),
                assertion: format!(
                    "expect(await page.evaluate(() => localStorage.getItem('{}'))).toBe('{}')",
                    k.replace('\'', "\\'"),
                    v.replace('\'', "\\'")
                ),
                source: format!("storage:{k}"),
            });
        }
        out
    }

    /// Emit Stryker + cargo-mutants hook manifests from covered test steps.
    pub fn mutation_hook_bridge(&self, steps: &[CoveredStep]) -> MutationHookManifest {
        let mut files: Vec<String> = steps.iter().map(|s| s.file.clone()).collect();
        files.sort();
        files.dedup();
        let mut mutants: Vec<String> = steps
            .iter()
            .flat_map(|s| s.mutation_targets.iter().cloned())
            .collect();
        mutants.sort();
        mutants.dedup();
        let stryker = serde_json::json!({
            "mutate": files,
            "testRunner": "command",
            "commandRunner": { "command": "npx playwright test" },
            "coveredSteps": steps.iter().map(|s| &s.id).collect::<Vec<_>>(),
        });
        let cargo_mutants = serde_json::json!({
            "examine": mutants,
            "test_tool": "cargo test",
            "timeout_multiplier": 2.0,
            "covered_steps": steps.len(),
        });
        MutationHookManifest {
            stryker,
            cargo_mutants,
            covered_steps: steps.len(),
        }
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
        assert!(std::path::Path::new(&exp.dir).join("frame-0000.html").exists());
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
        let net = FlakeVector::from_network_traces(&[
            NetworkTrace {
                method: "GET".into(),
                url: "https://x/a".into(),
                headers,
                body: None,
                status: 200,
            },
            NetworkTrace {
                method: "GET".into(),
                url: "https://x/a".into(),
                headers: headers2,
                body: None,
                status: 200,
            },
        ]);
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
        assert!(d.persist_ok, "evaluate_quarantine must persist yaml: {:?}", d.persist_err);
        assert!(d.persist_err.is_none());
        let path = QaEngine::quarantine_path();
        assert!(path.exists(), "expected yaml after evaluate at {}", path.display());
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
        assert!(readmit.persist_ok, "readmit persist: {:?}", readmit.persist_err);

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
        assert!(state["cookies"].as_array().unwrap().iter().any(|c| c["name"] == "sid"));
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
        obs.headers.insert("content-type".into(), "application/json".into());
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
