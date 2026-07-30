//! Performance SLA definitions and checker.
//!
//! Declarative Service Level Objectives (SLOs) grouped into an [`Sla`], plus an
//! [`SlaEngine`] that evaluates measured metrics (from [`crate::load`] runs)
//! against those objectives, computing pass/fail, margins and an availability
//! error budget. Bridges to [`crate::load::PerformanceBudgets`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::load::{BrowserMetrics, LoadMetrics, PerformanceBudgets};

/// Direction of an objective comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Comparison {
    /// Measured value must be <= threshold (latencies, error rate).
    AtMost,
    /// Measured value must be >= threshold (availability, throughput).
    AtLeast,
}

/// A single Service Level Objective.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaObjective {
    pub name: String,
    /// Metric key looked up in the measured metric map.
    pub metric: String,
    pub comparison: Comparison,
    pub threshold: f64,
    pub unit: String,
}

impl SlaObjective {
    pub fn at_most(name: &str, metric: &str, threshold: f64, unit: &str) -> Self {
        Self {
            name: name.into(),
            metric: metric.into(),
            comparison: Comparison::AtMost,
            threshold,
            unit: unit.into(),
        }
    }
    pub fn at_least(name: &str, metric: &str, threshold: f64, unit: &str) -> Self {
        Self {
            name: name.into(),
            metric: metric.into(),
            comparison: Comparison::AtLeast,
            threshold,
            unit: unit.into(),
        }
    }

    fn evaluate(&self, actual: f64) -> SlaObjectiveResult {
        let met = match self.comparison {
            Comparison::AtMost => actual <= self.threshold,
            Comparison::AtLeast => actual >= self.threshold,
        };
        // Positive margin = headroom, negative = breach amount.
        let margin = match self.comparison {
            Comparison::AtMost => self.threshold - actual,
            Comparison::AtLeast => actual - self.threshold,
        };
        SlaObjectiveResult {
            name: self.name.clone(),
            metric: self.metric.clone(),
            actual,
            threshold: self.threshold,
            unit: self.unit.clone(),
            met,
            margin,
        }
    }
}

/// A named collection of objectives forming an SLA.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sla {
    pub name: String,
    pub objectives: Vec<SlaObjective>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaObjectiveResult {
    pub name: String,
    pub metric: String,
    pub actual: f64,
    pub threshold: f64,
    pub unit: String,
    pub met: bool,
    pub margin: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaReport {
    pub sla: String,
    pub results: Vec<SlaObjectiveResult>,
    pub met_count: usize,
    pub total: usize,
    /// Objectives that had no corresponding measured metric.
    pub missing_metrics: Vec<String>,
    pub overall_met: bool,
}

/// Performance SLA engine: predefined SLAs plus a metric checker.
pub struct SlaEngine;

impl Default for SlaEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SlaEngine {
    pub fn new() -> Self {
        Self
    }

    /// Standard web-performance SLA aligned with Core Web Vitals "good" targets.
    pub fn web_performance_sla() -> Sla {
        Sla {
            name: "Web Performance".into(),
            objectives: vec![
                SlaObjective::at_most("LCP p75", "lcp_ms", 2500.0, "ms"),
                SlaObjective::at_most("INP p75", "inp_ms", 200.0, "ms"),
                SlaObjective::at_most("CLS", "cls", 0.1, ""),
                SlaObjective::at_most("TTFB", "ttfb_ms", 800.0, "ms"),
            ],
        }
    }

    /// Backend reliability SLA (latency + availability + throughput).
    pub fn reliability_sla() -> Sla {
        Sla {
            name: "Backend Reliability".into(),
            objectives: vec![
                SlaObjective::at_most("p95 latency", "p95_latency_ms", 500.0, "ms"),
                SlaObjective::at_most("p99 latency", "p99_latency_ms", 1000.0, "ms"),
                SlaObjective::at_least("availability", "availability_pct", 99.9, "%"),
                SlaObjective::at_most("error rate", "error_rate", 0.01, ""),
            ],
        }
    }

    pub fn standard_slas() -> Vec<Sla> {
        vec![Self::web_performance_sla(), Self::reliability_sla()]
    }

    /// Derive an SLA from declarative [`PerformanceBudgets`].
    pub fn from_budgets(budgets: &PerformanceBudgets) -> Sla {
        let mut objectives = Vec::new();
        let mut push_at_most = |name: &str, metric: &str, v: Option<f64>, unit: &str| {
            if let Some(t) = v {
                objectives.push(SlaObjective::at_most(name, metric, t, unit));
            }
        };
        push_at_most("LCP", "lcp_ms", budgets.max_lcp_ms, "ms");
        push_at_most("INP", "inp_ms", budgets.max_inp_ms, "ms");
        push_at_most("CLS", "cls", budgets.max_cls, "");
        push_at_most("TBT", "tbt_ms", budgets.max_tbt_ms, "ms");
        push_at_most("TTFB", "ttfb_ms", budgets.max_ttfb_ms, "ms");
        push_at_most(
            "p95 latency",
            "p95_latency_ms",
            budgets.max_p95_latency_ms,
            "ms",
        );
        push_at_most(
            "p99 latency",
            "p99_latency_ms",
            budgets.max_p99_latency_ms,
            "ms",
        );
        push_at_most("error rate", "error_rate", budgets.max_error_rate, "");
        if let Some(min_rps) = budgets.min_rps {
            objectives.push(SlaObjective::at_least("throughput", "rps", min_rps, "rps"));
        }
        Sla {
            name: "Budget SLA".into(),
            objectives,
        }
    }

    /// Flatten measured load + vitals into a metric map for SLA checking.
    pub fn metrics_map(
        load: Option<&LoadMetrics>,
        vitals: Option<&BrowserMetrics>,
    ) -> BTreeMap<String, f64> {
        let mut m = BTreeMap::new();
        if let Some(v) = vitals {
            m.insert("lcp_ms".into(), v.lcp_ms);
            m.insert("inp_ms".into(), v.inp_ms);
            m.insert("fid_ms".into(), v.fid_ms);
            m.insert("cls".into(), v.cls);
            m.insert("tbt_ms".into(), v.tbt_ms);
            m.insert("ttfb_ms".into(), v.ttfb_ms);
            m.insert("fcp_ms".into(), v.fcp_ms);
        }
        if let Some(l) = load {
            m.insert("p50_latency_ms".into(), l.p50_latency_ms);
            m.insert("p90_latency_ms".into(), l.p90_latency_ms);
            m.insert("p95_latency_ms".into(), l.p95_latency_ms);
            m.insert("p99_latency_ms".into(), l.p99_latency_ms);
            m.insert("rps".into(), l.rps);
            let error_rate = if l.total_requests > 0 {
                l.failed_requests as f64 / l.total_requests as f64
            } else {
                0.0
            };
            m.insert("error_rate".into(), error_rate);
            m.insert("availability_pct".into(), (1.0 - error_rate) * 100.0);
        }
        m
    }

    /// Evaluate an SLA against a measured metric map.
    pub fn check(&self, sla: &Sla, metrics: &BTreeMap<String, f64>) -> SlaReport {
        let mut results = Vec::new();
        let mut missing_metrics = Vec::new();
        let mut met_count = 0;
        for obj in &sla.objectives {
            match metrics.get(&obj.metric) {
                Some(actual) => {
                    let r = obj.evaluate(*actual);
                    if r.met {
                        met_count += 1;
                    }
                    results.push(r);
                }
                None => missing_metrics.push(obj.metric.clone()),
            }
        }
        let total = results.len();
        SlaReport {
            sla: sla.name.clone(),
            results,
            met_count,
            total,
            missing_metrics,
            overall_met: total > 0 && met_count == total,
        }
    }

    pub fn check_run(
        &self,
        sla: &Sla,
        load: Option<&LoadMetrics>,
        vitals: Option<&BrowserMetrics>,
    ) -> SlaReport {
        let metrics = Self::metrics_map(load, vitals);
        self.check(sla, &metrics)
    }

    /// Availability error budget remaining given an SLO target (e.g. 99.9%) and
    /// the observed availability. Returns the fraction of the allowed downtime
    /// budget still remaining, in `[0, 1]` (negative when the budget is blown).
    pub fn error_budget_remaining(slo_target_pct: f64, observed_pct: f64) -> f64 {
        let allowed_failure = (100.0 - slo_target_pct).max(f64::EPSILON) / 100.0;
        let observed_failure = (100.0 - observed_pct).max(0.0) / 100.0;
        1.0 - (observed_failure / allowed_failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vitals(lcp: f64, inp: f64, cls: f64, ttfb: f64) -> BrowserMetrics {
        BrowserMetrics {
            lcp_ms: lcp,
            inp_ms: inp,
            cls,
            ttfb_ms: ttfb,
            ..Default::default()
        }
    }

    #[test]
    fn test_web_perf_sla_pass_and_fail() {
        let engine = SlaEngine::new();
        let sla = SlaEngine::web_performance_sla();
        let good = vitals(2000.0, 150.0, 0.05, 500.0);
        let report = engine.check_run(&sla, None, Some(&good));
        assert!(report.overall_met);
        assert_eq!(report.met_count, 4);

        let bad = vitals(3000.0, 150.0, 0.05, 500.0);
        let report = engine.check_run(&sla, None, Some(&bad));
        assert!(!report.overall_met);
        assert!(report
            .results
            .iter()
            .any(|r| r.metric == "lcp_ms" && !r.met));
    }

    #[test]
    fn test_missing_metrics_tracked() {
        let engine = SlaEngine::new();
        let sla = SlaEngine::reliability_sla();
        let report = engine.check(&sla, &BTreeMap::new());
        assert_eq!(report.missing_metrics.len(), sla.objectives.len());
        assert!(!report.overall_met);
    }

    #[test]
    fn test_from_budgets() {
        let budgets = PerformanceBudgets::core_web_vitals();
        let sla = SlaEngine::from_budgets(&budgets);
        assert!(sla.objectives.iter().any(|o| o.metric == "lcp_ms"));
        assert!(sla.objectives.iter().any(|o| o.metric == "error_rate"));
    }

    #[test]
    fn test_error_budget() {
        // 99.9% target, observed 99.95% -> half the budget remains.
        let remaining = SlaEngine::error_budget_remaining(99.9, 99.95);
        assert!((remaining - 0.5).abs() < 0.01);
        // Observed worse than target -> negative budget.
        assert!(SlaEngine::error_budget_remaining(99.9, 99.0) < 0.0);
    }

    #[test]
    fn test_reliability_from_load_metrics() {
        let engine = SlaEngine::new();
        let load = LoadMetrics {
            total_requests: 1000,
            successful_requests: 999,
            failed_requests: 1,
            total_bytes: 0,
            duration_secs: 10.0,
            rps: 100.0,
            min_latency_ms: 10.0,
            max_latency_ms: 400.0,
            avg_latency_ms: 100.0,
            p50_latency_ms: 90.0,
            p90_latency_ms: 200.0,
            p95_latency_ms: 300.0,
            p99_latency_ms: 800.0,
            status_codes: std::collections::HashMap::new(),
            browser_vitals: None,
            transfer_bytes: None,
            decoded_bytes: None,
            lcp_media_bytes: None,
            soft_nav: None,
        };
        let report = engine.check_run(&SlaEngine::reliability_sla(), Some(&load), None);
        assert!(report.overall_met, "results: {:?}", report.results);
    }
}

// ===========================================================================
// Feature-path latency SLAs
//
// The SLOs above judge *measured runtime metrics* (LCP, latency percentiles,
// availability) coming out of load runs. This section adds hard latency budgets
// for Thor's *in-process critical feature paths* — the hot developer-workflow
// operations whose speed defines the "lighter/faster-than-Chrome" claim.
//
// Each [`FeaturePathSla`] is a single source of truth shared by:
//   * the `#[cfg(test)]` enforcement tests below (fail the build if a path
//     regresses past its budget in the test environment), and
//   * `benches/feature_sla_bench.rs` (Criterion micro-benchmarks + summary).
//
// Budgets are stated for release builds; [`FeaturePathSla::effective_threshold`]
// grants slack under `debug_assertions`. Measurements take the best (min)
// amortized per-op time to stay robust against scheduler noise, and every path
// is wired to a real engine (no mocks) per AGENTS.md.
// ===========================================================================

use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::devtools::parse_web_vitals;
use crate::dx::llm_studio::LlmStudio;
use crate::dx::test_generator::{DomStep, NetworkTrace, RecordedSession, TestGenerator};
use crate::features::backend::{BackendEngine, IdorProbe, WebhookEvent};
use crate::features::cdp_bridge::CdpBridge;
use crate::features::perf_features::{
    ConnPoolEntry, LcpElementPayload, LongTaskEvent, PerfEngine, StackSample,
};
use crate::features::security::{AuthzProbe, HttpSnapshot, SecurityEngine};
use crate::features::{FeatureEngine, FeatureId, FeatureRegistry};
use crate::load::LatencyHistogram;

/// Extra latency headroom allowed for unoptimized `debug_assertions` builds,
/// where the same code runs far slower than the `--release` budget assumes.
pub const DEBUG_SLACK: u32 = 25;

/// A hard latency budget for one in-process critical feature path.
#[derive(Debug, Clone, Copy)]
pub struct FeaturePathSla {
    /// Stable SLA id.
    pub id: &'static str,
    /// The registered feature this budget guards.
    pub feature: FeatureId,
    /// Human description including the unit of work.
    pub description: &'static str,
    /// Release-build latency budget for one unit of work.
    pub threshold: Duration,
}

impl FeaturePathSla {
    /// Budget adjusted for the current build profile.
    pub fn effective_threshold(&self) -> Duration {
        if cfg!(debug_assertions) {
            self.threshold * DEBUG_SLACK
        } else {
            self.threshold
        }
    }
}

pub const SLA_FEATURE_REGISTRY_LOOKUP: FeaturePathSla = FeaturePathSla {
    id: "feature_registry_lookup",
    feature: FeatureId::PerformanceSla,
    description: "FeatureRegistry lookup (single slug)",
    threshold: Duration::from_nanos(1_000), // 1µs
};

pub const SLA_HDR_PERCENTILE_10K: FeaturePathSla = FeaturePathSla {
    id: "hdr_percentile_10k",
    feature: FeatureId::HdrHistogramHelper,
    description: "HDR histogram: record 10k samples + p50/p90/p99",
    threshold: Duration::from_millis(5),
};

pub const SLA_AUTHZ_MATRIX_4X1: FeaturePathSla = FeaturePathSla {
    id: "authz_matrix_4x1",
    feature: FeatureId::AuthzMatrix,
    description: "Authz/IDOR matrix (4 contexts x 1 request)",
    threshold: Duration::from_millis(50),
};

pub const SLA_JWT_DEEP_ANALYSIS: FeaturePathSla = FeaturePathSla {
    id: "jwt_deep_analysis",
    feature: FeatureId::JwtDeep,
    description: "JWT decode + deep security analysis",
    threshold: Duration::from_millis(2),
};

pub const SLA_PLAYWRIGHT_EXPORT_100: FeaturePathSla = FeaturePathSla {
    id: "playwright_export_100",
    feature: FeatureId::DeflakedExport,
    description: "Promote to Playwright export (100 steps)",
    threshold: Duration::from_millis(10),
};

pub const SLA_LLM_COST_ESTIMATE: FeaturePathSla = FeaturePathSla {
    id: "llm_cost_estimate",
    feature: FeatureId::CostDashboard,
    description: "LLM API cost estimate (single call)",
    threshold: Duration::from_millis(1),
};

pub const SLA_RESPONSE_DIFF: FeaturePathSla = FeaturePathSla {
    id: "response_diff",
    feature: FeatureId::ResponseDiff,
    description: "Response normalize + structured diff",
    threshold: Duration::from_millis(5),
};

// --- v2 critical paths: orchestrator routing, CDP bridge, backend sinks ------

pub const SLA_FEATURE_INVOKE_ROUTING: FeaturePathSla = FeaturePathSla {
    id: "feature_invoke_routing",
    feature: FeatureId::PerformanceSla,
    description: "Orchestrator feature-invoke routing (slug -> engine)",
    threshold: Duration::from_micros(500), // 500µs
};

pub const SLA_CDP_PARSE_WEB_VITALS: FeaturePathSla = FeaturePathSla {
    id: "cdp_parse_web_vitals",
    feature: FeatureId::VitalsUnderLoad,
    description: "CDP bridge parse Core Web Vitals JSON",
    threshold: Duration::from_millis(1),
};

pub const SLA_WEBHOOK_SINK_RECORD: FeaturePathSla = FeaturePathSla {
    id: "webhook_sink_record",
    feature: FeatureId::WebhookSink,
    description: "Webhook sink record event",
    threshold: Duration::from_millis(5),
};

pub const SLA_IDOR_MATRIX_4CTX: FeaturePathSla = FeaturePathSla {
    id: "idor_matrix_4ctx",
    feature: FeatureId::IdorMatrix,
    description: "IDOR matrix analyze (4 contexts x 1 resource)",
    threshold: Duration::from_millis(100),
};

pub const SLA_LCP_ELEMENT_FORENSICS: FeaturePathSla = FeaturePathSla {
    id: "lcp_element_forensics",
    feature: FeatureId::LcpElementForensics,
    description: "LCP element forensics from vitals JSON payload",
    threshold: Duration::from_micros(200),
};

pub const SLA_LONG_TASK_RADAR: FeaturePathSla = FeaturePathSla {
    id: "long_task_radar_1k",
    feature: FeatureId::LongTaskRadar,
    description: "Long-task radar rank 1k events",
    threshold: Duration::from_millis(5),
};

pub const SLA_CONN_POOL_INSPECTOR: FeaturePathSla = FeaturePathSla {
    id: "conn_pool_inspector_1k",
    feature: FeatureId::ConnPoolInspector,
    description: "Connection pool inspect 1k entries",
    threshold: Duration::from_millis(5),
};

pub const SLA_CONTINUOUS_PROFILER: FeaturePathSla = FeaturePathSla {
    id: "continuous_profiler_500",
    feature: FeatureId::ContinuousProfiler,
    description: "Continuous profiler fold 500 stack samples",
    threshold: Duration::from_millis(10),
};

pub const SLA_CDP_INGEST_NETWORK: FeaturePathSla = FeaturePathSla {
    id: "cdp_ingest_network_batch",
    feature: FeatureId::ConnPoolInspector,
    description: "CDP Network JSON ingest → ConnPool/Http3/Compression",
    threshold: Duration::from_millis(2),
};

pub const SLA_BUDGET_CHECK_UNIFIED: FeaturePathSla = FeaturePathSla {
    id: "budget_check_unified",
    feature: FeatureId::PerformanceSla,
    description: "PerfEngine + PerformanceBudgets::evaluate same fixture",
    threshold: Duration::from_micros(50),
};

/// Every feature-path SLA, in reporting order.
pub const FEATURE_PATH_SLAS: &[FeaturePathSla] = &[
    SLA_FEATURE_REGISTRY_LOOKUP,
    SLA_HDR_PERCENTILE_10K,
    SLA_AUTHZ_MATRIX_4X1,
    SLA_JWT_DEEP_ANALYSIS,
    SLA_PLAYWRIGHT_EXPORT_100,
    SLA_LLM_COST_ESTIMATE,
    SLA_RESPONSE_DIFF,
    SLA_FEATURE_INVOKE_ROUTING,
    SLA_CDP_PARSE_WEB_VITALS,
    SLA_WEBHOOK_SINK_RECORD,
    SLA_IDOR_MATRIX_4CTX,
    SLA_LCP_ELEMENT_FORENSICS,
    SLA_LONG_TASK_RADAR,
    SLA_CONN_POOL_INSPECTOR,
    SLA_CONTINUOUS_PROFILER,
    SLA_CDP_INGEST_NETWORK,
    SLA_BUDGET_CHECK_UNIFIED,
];

/// Result of measuring one feature-path SLA.
#[derive(Debug, Clone, Copy)]
pub struct FeaturePathResult {
    pub sla: FeaturePathSla,
    /// Best observed amortized per-operation latency.
    pub per_op: Duration,
    pub passed: bool,
}

impl FeaturePathResult {
    pub fn effective_threshold(&self) -> Duration {
        self.sla.effective_threshold()
    }
}

// --- Shared workload fixtures (reused by the Criterion bench) ---------------

/// A representative HS256 JWT.
/// header {"alg":"HS256","typ":"JWT","kid":"key-1"} ·
/// payload {"sub":"user-42","iss":"thor","iat":1700000000,"exp":1700003600,"name":"Thor Security"}
pub const SAMPLE_JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImtleS0xIn0.eyJzdWIiOiJ1c2VyLTQyIiwiaXNzIjoidGhvciIsImlhdCI6MTcwMDAwMDAwMCwiZXhwIjoxNzAwMDAzNjAwLCJuYW1lIjoiVGhvciBTZWN1cml0eSJ9.c2lnbmF0dXJlX2J5dGVz";

/// Deterministic latency sample set (ms) with a long tail for the histogram.
pub fn hdr_ms_samples(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let base = 2.0 + (i as f64 % 800.0) * 0.3;
            if i % 97 == 0 {
                base * 12.0
            } else {
                base
            }
        })
        .collect()
}

/// The canonical 4-context authorization probe set for a single owner-scoped
/// endpoint (Admin + Owner legitimately allowed; Other User + Anonymous must be
/// denied). `status` simulates a vulnerable endpoint that grants everyone.
pub fn authz_probes() -> Vec<AuthzProbe> {
    let endpoint = "/api/orders/1001";
    vec![
        AuthzProbe {
            role: "Admin".into(),
            endpoint: endpoint.into(),
            should_allow: true,
            status: 200,
        },
        AuthzProbe {
            role: "Owner".into(),
            endpoint: endpoint.into(),
            should_allow: true,
            status: 200,
        },
        AuthzProbe {
            role: "Other User".into(),
            endpoint: endpoint.into(),
            should_allow: false,
            status: 200,
        },
        AuthzProbe {
            role: "Anonymous".into(),
            endpoint: endpoint.into(),
            should_allow: false,
            status: 200,
        },
    ]
}

/// A recorded session with `steps` DOM interactions + periodic network traces.
pub fn playwright_session(steps: usize) -> RecordedSession {
    let mut session = RecordedSession::new("SLA Export Flow", "https://app.thor.dev/login");
    for i in 0..steps {
        let step = match i % 4 {
            0 => DomStep::new("click", format!("#btn-{i}")),
            1 => DomStep::new("fill", format!("#field-{i}")).with_value(format!("value-{i}")),
            2 => DomStep::new("navigate", format!("https://app.thor.dev/step/{i}")),
            _ => DomStep::new("wait", "250"),
        };
        session.add_dom_step(step);
        if i % 10 == 0 {
            session.add_trace(NetworkTrace {
                method: "POST".into(),
                url: format!("https://api.thor.dev/v1/action/{i}"),
                headers: [("Content-Type".to_string(), "application/json".to_string())]
                    .into_iter()
                    .collect(),
                body: Some(format!("{{\"step\":{i}}}")),
                status: 200,
            });
        }
    }
    session
}

/// A pair of multi-line JSON response snapshots that differ in a few fields, for
/// the response-diff path.
pub fn response_snapshots() -> (HttpSnapshot, HttpSnapshot) {
    let mut a_body = String::new();
    let mut b_body = String::new();
    for i in 0..50 {
        a_body.push_str(&format!(
            "  \"item_{i}\": {{ \"price\": {}, \"qty\": 1 }},\n",
            100 + i
        ));
        let price = if i % 10 == 0 { 100 + i + 5 } else { 100 + i };
        b_body.push_str(&format!(
            "  \"item_{i}\": {{ \"price\": {price}, \"qty\": 1 }},\n"
        ));
    }
    let headers_a: BTreeMap<String, String> = [
        ("content-type".to_string(), "application/json".to_string()),
        ("x-request-id".to_string(), "aaa".to_string()),
    ]
    .into_iter()
    .collect();
    let headers_b: BTreeMap<String, String> = [
        ("content-type".to_string(), "application/json".to_string()),
        ("x-request-id".to_string(), "zzz".to_string()),
    ]
    .into_iter()
    .collect();
    (
        HttpSnapshot {
            status: 200,
            headers: headers_a,
            body: a_body,
        },
        HttpSnapshot {
            status: 200,
            headers: headers_b,
            body: b_body,
        },
    )
}

/// A representative Core Web Vitals JSON payload as emitted by the CDP bridge's
/// `Runtime.evaluate` collection script (flat object, short web-vitals keys).
pub const SAMPLE_WEB_VITALS_JSON: &str = r#"{"lcp":2100.5,"fid":12.0,"inp":150.0,"tbt":120.0,"cls":0.03,"fcp":900.0,"ttfb":420.0,"domContentLoaded":1100.0,"domInteractive":1000.0}"#;

/// A representative inbound webhook event for the backend webhook sink.
pub fn sample_webhook_event() -> WebhookEvent {
    WebhookEvent {
        method: "POST".into(),
        path: "/hooks/stripe/payment_intent".into(),
        headers: [
            ("content-type".to_string(), "application/json".to_string()),
            (
                "x-signature".to_string(),
                "t=1700000000,v1=abc123def456".to_string(),
            ),
        ]
        .into_iter()
        .collect(),
        body: r#"{"id":"evt_1","type":"payment_intent.succeeded","amount":4200}"#.into(),
        received_ms: 1_700_000_000_000,
    }
}

/// The canonical 4-context IDOR/BOLA probe set for a single owner-scoped
/// resource (Admin + Owner legitimately allowed; Other User granted a 2xx =
/// broken object-level authz; Anonymous correctly denied).
pub fn idor_probes() -> Vec<IdorProbe> {
    let resource = "/api/orders/1001";
    vec![
        IdorProbe {
            identity: "Admin".into(),
            resource: resource.into(),
            is_owner: true,
            status: 200,
        },
        IdorProbe {
            identity: "Owner".into(),
            resource: resource.into(),
            is_owner: true,
            status: 200,
        },
        IdorProbe {
            identity: "Other User".into(),
            resource: resource.into(),
            is_owner: false,
            status: 200,
        },
        IdorProbe {
            identity: "Anonymous".into(),
            resource: resource.into(),
            is_owner: false,
            status: 401,
        },
    ]
}

// --- Measurement harness ---------------------------------------------------

/// Best (min) amortized per-op latency of `f` over `repeats` batches of `iters`
/// invocations each, with a warmup pass.
fn measure_per_op<F: FnMut()>(repeats: u32, iters: u64, mut f: F) -> Duration {
    let warmup = (iters / 10).max(1);
    for _ in 0..warmup {
        f();
    }
    let mut best = Duration::MAX;
    for _ in 0..repeats {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        let per = start.elapsed() / iters as u32;
        if per < best {
            best = per;
        }
    }
    best
}

fn result(sla: FeaturePathSla, per_op: Duration) -> FeaturePathResult {
    FeaturePathResult {
        passed: per_op <= sla.effective_threshold(),
        sla,
        per_op,
    }
}

pub fn measure_registry_lookup() -> FeaturePathResult {
    let registry = FeatureRegistry::new();
    let slugs = [
        "response-diff",
        "authz-matrix",
        "jwt-deep",
        "cost-dashboard",
        "hdr-histogram-helper",
    ];
    let mut k = 0usize;
    let per_op = measure_per_op(5, 200_000, || {
        let slug = slugs[k % slugs.len()];
        k += 1;
        black_box(registry.by_slug(black_box(slug)));
    });
    result(SLA_FEATURE_REGISTRY_LOOKUP, per_op)
}

pub fn measure_hdr_percentiles() -> FeaturePathResult {
    let samples = hdr_ms_samples(10_000);
    let per_op = measure_per_op(5, 50, || {
        let mut h = LatencyHistogram::default();
        for &s in &samples {
            h.record(s);
        }
        black_box(h.percentile(50.0));
        black_box(h.percentile(90.0));
        black_box(h.percentile(99.0));
    });
    result(SLA_HDR_PERCENTILE_10K, per_op)
}

pub fn measure_authz_matrix() -> FeaturePathResult {
    let engine = SecurityEngine::new();
    let probes = authz_probes();
    let per_op = measure_per_op(5, 20_000, || {
        black_box(engine.build_authz_matrix(black_box(&probes)));
    });
    result(SLA_AUTHZ_MATRIX_4X1, per_op)
}

pub fn measure_jwt_analysis() -> FeaturePathResult {
    let engine = SecurityEngine::new();
    let per_op = measure_per_op(5, 20_000, || {
        black_box(
            engine
                .inspect_jwt(black_box(SAMPLE_JWT), 1_700_002_000)
                .unwrap(),
        );
    });
    result(SLA_JWT_DEEP_ANALYSIS, per_op)
}

pub fn measure_playwright_export() -> FeaturePathResult {
    let session = playwright_session(100);
    let per_op = measure_per_op(5, 2_000, || {
        black_box(TestGenerator::export_playwright_ts(black_box(&session)));
    });
    result(SLA_PLAYWRIGHT_EXPORT_100, per_op)
}

pub fn measure_llm_cost() -> FeaturePathResult {
    let per_op = measure_per_op(5, 200_000, || {
        black_box(LlmStudio::estimate_cost(
            black_box("claude-3.5-sonnet"),
            black_box(12_000),
            black_box(3_500),
        ));
    });
    result(SLA_LLM_COST_ESTIMATE, per_op)
}

pub fn measure_response_diff() -> FeaturePathResult {
    let engine = SecurityEngine::new();
    let (a, b) = response_snapshots();
    let per_op = measure_per_op(5, 2_000, || {
        black_box(engine.diff_responses(black_box(&a), black_box(&b)));
    });
    result(SLA_RESPONSE_DIFF, per_op)
}

pub fn measure_feature_invoke_routing() -> FeaturePathResult {
    let engine = FeatureEngine::new();
    let slugs = [
        "response-diff",
        "authz-matrix",
        "jwt-deep",
        "webhook-sink",
        "idor-matrix",
    ];
    let mut k = 0usize;
    let per_op = measure_per_op(5, 200_000, || {
        let slug = slugs[k % slugs.len()];
        k += 1;
        black_box(engine.invoke(black_box(slug)).unwrap());
    });
    result(SLA_FEATURE_INVOKE_ROUTING, per_op)
}

pub fn measure_cdp_parse_web_vitals() -> FeaturePathResult {
    let per_op = measure_per_op(5, 100_000, || {
        black_box(parse_web_vitals(black_box(SAMPLE_WEB_VITALS_JSON)).unwrap());
    });
    result(SLA_CDP_PARSE_WEB_VITALS, per_op)
}

pub fn measure_webhook_sink_record() -> FeaturePathResult {
    let engine = BackendEngine::new();
    let event = sample_webhook_event();
    let per_op = measure_per_op(5, 10_000, || {
        engine.record_webhook(black_box(event.clone()));
    });
    result(SLA_WEBHOOK_SINK_RECORD, per_op)
}

pub fn measure_idor_matrix() -> FeaturePathResult {
    let engine = BackendEngine::new();
    let probes = idor_probes();
    let per_op = measure_per_op(5, 20_000, || {
        black_box(engine.analyze_idor(black_box(&probes)));
    });
    result(SLA_IDOR_MATRIX_4CTX, per_op)
}

pub fn measure_lcp_element_forensics() -> FeaturePathResult {
    let engine = PerfEngine::new();
    let payload = LcpElementPayload {
        vitals_json: Some(
            r#"{"lcp":2100,"lcpElement":{"url":"https://cdn/hero.webp","type":"image","size":82000,"discoverTime":120,"loadTime":900,"renderTime":2100}}"#
                .into(),
        ),
        ..Default::default()
    };
    let per_op = measure_per_op(5, 50_000, || {
        black_box(engine.lcp_element_forensics(black_box(&payload)));
    });
    result(SLA_LCP_ELEMENT_FORENSICS, per_op)
}

pub fn measure_long_task_radar() -> FeaturePathResult {
    let engine = PerfEngine::new();
    let tasks: Vec<LongTaskEvent> = (0..1_000)
        .map(|i| LongTaskEvent {
            duration_ms: 60.0 + (i % 40) as f64,
            start_ms: i as f64,
            script_url: Some(format!("https://cdn/chunk-{}.js", i % 17)),
            name: Some("script".into()),
            inp_overlap: i % 11 == 0,
        })
        .collect();
    let per_op = measure_per_op(5, 200, || {
        black_box(engine.long_task_radar(black_box(&tasks)));
    });
    result(SLA_LONG_TASK_RADAR, per_op)
}

pub fn measure_conn_pool_inspector() -> FeaturePathResult {
    let engine = PerfEngine::new();
    let entries: Vec<ConnPoolEntry> = (0..1_000)
        .map(|i| ConnPoolEntry {
            url: format!("https://api.example.com/{i}"),
            connection_id: format!("c{}", i % 32),
            wait_ms: (i % 25) as f64,
            reused: i % 3 != 0,
            protocol: Some(if i % 5 == 0 { "h3" } else { "h2" }.into()),
        })
        .collect();
    let per_op = measure_per_op(5, 200, || {
        black_box(engine.conn_pool_inspect(black_box(&entries)));
    });
    result(SLA_CONN_POOL_INSPECTOR, per_op)
}

pub fn measure_continuous_profiler() -> FeaturePathResult {
    let engine = PerfEngine::new();
    let samples: Vec<StackSample> = (0..500)
        .map(|i| StackSample {
            stack: vec![
                "main".into(),
                format!("fn_{}", i % 20),
                format!("leaf_{}", i % 7),
            ],
            weight: 1 + (i % 5) as u64,
        })
        .collect();
    let per_op = measure_per_op(5, 50, || {
        black_box(engine.continuous_profile(black_box(&samples)));
    });
    result(SLA_CONTINUOUS_PROFILER, per_op)
}

pub const SAMPLE_CDP_NETWORK_JSON: &str = r#"[
  {"url":"https://cdn.example.com/a.js","status":200,"protocol":"h3","connectionId":7,"connectionReused":false,"encodedDataLength":800,"decodedBodyLength":800,"headers":{"content-encoding":"br"}},
  {"url":"https://cdn.example.com/b.js","status":200,"protocol":"h3","connectionId":7,"connectionReused":true,"encodedDataLength":400,"decodedBodyLength":1600,"headers":{"content-encoding":"br"}}
]"#;

pub fn measure_cdp_ingest_network() -> FeaturePathResult {
    let per_op = measure_per_op(5, 5_000, || {
        let bridge = CdpBridge::new();
        black_box(bridge.ingest_cdp_network_json(black_box(SAMPLE_CDP_NETWORK_JSON)));
        let pool = bridge.conn_pool_entries();
        let h3 = bridge.http3_path_samples();
        let comp = bridge.compression_entries();
        let engine = PerfEngine::new();
        black_box(engine.conn_pool_inspect(black_box(&pool)));
        black_box(engine.http3_path_audit(black_box(&h3)));
        black_box(engine.compression_budget(black_box(&comp), 4.0));
    });
    result(SLA_CDP_INGEST_NETWORK, per_op)
}

pub fn measure_budget_check_unified() -> FeaturePathResult {
    let engine = PerfEngine::new();
    let budgets = PerformanceBudgets::core_web_vitals();
    let vitals = BrowserMetrics {
        lcp_ms: 1800.0,
        inp_ms: 120.0,
        cls: 0.05,
        tbt_ms: 100.0,
        ttfb_ms: 400.0,
        ..Default::default()
    };
    let load = LoadMetrics {
        total_requests: 1000,
        successful_requests: 995,
        failed_requests: 5,
        total_bytes: 0,
        duration_secs: 10.0,
        rps: 100.0,
        min_latency_ms: 1.0,
        max_latency_ms: 200.0,
        avg_latency_ms: 50.0,
        p50_latency_ms: 40.0,
        p90_latency_ms: 80.0,
        p95_latency_ms: 100.0,
        p99_latency_ms: 150.0,
        status_codes: Default::default(),
        browser_vitals: Some(vitals.clone()),
        transfer_bytes: None,
        decoded_bytes: None,
        lcp_media_bytes: None,
        soft_nav: None,
    };
    let per_op = measure_per_op(5, 20_000, || {
        let pe = engine.check_budgets(
            black_box(&budgets),
            Some(black_box(&vitals)),
            Some(black_box(&load)),
        );
        let eval = budgets.evaluate(
            Some(black_box(&vitals)),
            Some(black_box(&load)),
            None,
            None,
            None,
        );
        black_box((pe.passed == eval.passed, pe.checked == eval.checked));
    });
    result(SLA_BUDGET_CHECK_UNIFIED, per_op)
}

/// Run every feature-path SLA and return one result per target (in
/// [`FEATURE_PATH_SLAS`] order).
pub fn run_feature_path_slas() -> Vec<FeaturePathResult> {
    vec![
        measure_registry_lookup(),
        measure_hdr_percentiles(),
        measure_authz_matrix(),
        measure_jwt_analysis(),
        measure_playwright_export(),
        measure_llm_cost(),
        measure_response_diff(),
        measure_feature_invoke_routing(),
        measure_cdp_parse_web_vitals(),
        measure_webhook_sink_record(),
        measure_idor_matrix(),
        measure_lcp_element_forensics(),
        measure_long_task_radar(),
        measure_conn_pool_inspector(),
        measure_continuous_profiler(),
        measure_cdp_ingest_network(),
        measure_budget_check_unified(),
    ]
}

impl SlaEngine {
    /// The declarative feature-path latency budgets.
    pub fn feature_path_slas(&self) -> &'static [FeaturePathSla] {
        FEATURE_PATH_SLAS
    }

    /// Measure every feature-path SLA against its budget.
    pub fn measure_feature_paths(&self) -> Vec<FeaturePathResult> {
        run_feature_path_slas()
    }
}

#[cfg(test)]
mod feature_path_sla_tests {
    use super::*;

    fn assert_sla(r: FeaturePathResult) {
        println!(
            "SLA [{}] {}: {:?} (budget {:?}{}) -> {}",
            r.sla.id,
            r.sla.description,
            r.per_op,
            r.effective_threshold(),
            if cfg!(debug_assertions) {
                format!(", {DEBUG_SLACK}x debug slack")
            } else {
                String::new()
            },
            if r.passed { "PASS" } else { "FAIL" }
        );
        assert!(
            r.passed,
            "SLA VIOLATION [{}]: {} took {:?}, budget {:?}",
            r.sla.id,
            r.sla.description,
            r.per_op,
            r.effective_threshold()
        );
    }

    #[test]
    fn sla_feature_registry_lookup() {
        assert_sla(measure_registry_lookup());
    }

    #[test]
    fn sla_hdr_percentiles_10k() {
        assert_sla(measure_hdr_percentiles());
    }

    #[test]
    fn sla_authz_matrix_4x1() {
        assert_sla(measure_authz_matrix());
    }

    #[test]
    fn sla_jwt_deep_analysis() {
        assert_sla(measure_jwt_analysis());
    }

    #[test]
    fn sla_playwright_export_100() {
        assert_sla(measure_playwright_export());
    }

    #[test]
    fn sla_llm_cost_estimate() {
        assert_sla(measure_llm_cost());
    }

    #[test]
    fn sla_response_diff() {
        assert_sla(measure_response_diff());
    }

    #[test]
    fn sla_feature_invoke_routing() {
        assert_sla(measure_feature_invoke_routing());
    }

    #[test]
    fn sla_cdp_parse_web_vitals() {
        assert_sla(measure_cdp_parse_web_vitals());
    }

    #[test]
    fn sla_webhook_sink_record() {
        assert_sla(measure_webhook_sink_record());
    }

    #[test]
    fn sla_idor_matrix_4ctx() {
        assert_sla(measure_idor_matrix());
    }

    #[test]
    fn sla_lcp_element_forensics() {
        assert_sla(measure_lcp_element_forensics());
    }

    #[test]
    fn sla_long_task_radar_1k() {
        assert_sla(measure_long_task_radar());
    }

    #[test]
    fn sla_conn_pool_inspector_1k() {
        assert_sla(measure_conn_pool_inspector());
    }

    #[test]
    fn sla_continuous_profiler_500() {
        assert_sla(measure_continuous_profiler());
    }

    #[test]
    fn sla_cdp_ingest_network_batch() {
        assert_sla(measure_cdp_ingest_network());
    }

    #[test]
    fn sla_budget_check_unified() {
        assert_sla(measure_budget_check_unified());
    }

    #[test]
    fn every_feature_path_sla_maps_to_a_registered_feature() {
        let registry = FeatureRegistry::new();
        for s in FEATURE_PATH_SLAS {
            assert!(
                registry.get(s.feature).is_some(),
                "SLA {} references unregistered feature {:?}",
                s.id,
                s.feature
            );
        }
    }

    #[test]
    fn full_feature_path_suite_runs() {
        let reports = run_feature_path_slas();
        assert_eq!(reports.len(), FEATURE_PATH_SLAS.len());
    }
}
