use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::load::metrics::{BrowserMetrics, LoadMetrics};

/// One structured budget gate failure (shared by load + PerfEngine).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetGate {
    pub metric: String,
    pub actual: f64,
    pub budget: f64,
    pub over_by: f64,
}

/// Result of evaluating [`PerformanceBudgets`] — single source of truth for
/// `PerformanceBudgets::check` and `PerfEngine::check_budgets*`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetEvaluation {
    pub passed: bool,
    pub violations: Vec<BudgetGate>,
    pub checked: usize,
}

/// Runner execution engine target
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunnerType {
    /// Chromium Web Vitals and DOM behavior test runner
    Browser,
    /// High-throughput multi-threaded Rust HTTP/WS scale test runner
    Protocol,
    /// Dual hybrid runner combining browser UI probes with protocol scale
    Hybrid,
}

impl Default for RunnerType {
    fn default() -> Self {
        RunnerType::Protocol
    }
}

/// Individual step within a load scenario
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScenarioStep {
    /// Browser/Protocol navigation to target URL
    Navigate { url: String },
    /// Protocol HTTP request specification
    HttpRequest {
        method: String,
        url: String,
        headers: HashMap<String, String>,
        body: Option<String>,
    },
    /// Browser DOM element click action
    Click { selector: String },
    /// Browser DOM element input fill action
    Fill { selector: String, value: String },
    /// Think time / sleep step in milliseconds
    Wait { duration_ms: u64 },
    /// WebSocket load connection & message sequence
    WebSocketConnect { url: String, messages: Vec<String> },
    /// Custom JavaScript or script step execution
    CustomScript { name: String, script: String },
}

/// Performance budgets asserted against a scenario run. Any `None` field is not
/// enforced. Thresholds are pass/fail gates used by CI and the perf SLA checker.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PerformanceBudgets {
    /// Max acceptable p75 Largest Contentful Paint (ms).
    pub max_lcp_ms: Option<f64>,
    /// Max acceptable Interaction to Next Paint (ms).
    pub max_inp_ms: Option<f64>,
    /// Max acceptable Cumulative Layout Shift (unitless).
    pub max_cls: Option<f64>,
    /// Max acceptable Total Blocking Time (ms).
    pub max_tbt_ms: Option<f64>,
    /// Max acceptable Time to First Byte (ms).
    pub max_ttfb_ms: Option<f64>,
    /// Max acceptable p95 request latency (ms).
    pub max_p95_latency_ms: Option<f64>,
    /// Max acceptable p99 request latency (ms).
    pub max_p99_latency_ms: Option<f64>,
    /// Max acceptable error rate as a fraction in [0,1].
    pub max_error_rate: Option<f64>,
    /// Minimum acceptable throughput (requests/sec).
    pub min_rps: Option<f64>,
    /// Max total transfer size across audited resources (bytes).
    #[serde(default)]
    pub max_transfer_bytes: Option<u64>,
    /// Max decoded/transfer inflation ratio (e.g. 4.0 = 4×).
    #[serde(default)]
    pub max_decoded_inflation_ratio: Option<f64>,
    /// Max bytes for the LCP-contributing media resource.
    #[serde(default)]
    pub max_lcp_media_bytes: Option<u64>,
}

impl PerformanceBudgets {
    /// Web-vitals "good" thresholds (2024 Core Web Vitals) as a starting budget.
    pub fn core_web_vitals() -> Self {
        Self {
            max_lcp_ms: Some(2500.0),
            max_inp_ms: Some(200.0),
            max_cls: Some(0.1),
            max_tbt_ms: Some(200.0),
            max_ttfb_ms: Some(800.0),
            max_p95_latency_ms: None,
            max_p99_latency_ms: None,
            max_error_rate: Some(0.01),
            min_rps: None,
            max_transfer_bytes: None,
            max_decoded_inflation_ratio: Some(5.0),
            max_lcp_media_bytes: Some(500_000),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == PerformanceBudgets::default()
    }

    /// Canonical budget evaluation used by load runners and
    /// [`crate::features::perf_features::PerfEngine::check_budgets`].
    ///
    /// Budget fields left as `None` are not enforced. Web-vitals gates are only
    /// checked when `vitals` is `Some`, so a protocol-only run is never failed
    /// for a missing LCP measurement.
    pub fn evaluate(
        &self,
        vitals: Option<&BrowserMetrics>,
        load: Option<&LoadMetrics>,
        transfer_bytes: Option<u64>,
        decoded_bytes: Option<u64>,
        lcp_media_bytes: Option<u64>,
    ) -> BudgetEvaluation {
        let mut violations = Vec::new();
        let mut checked = 0;
        let mut gate = |metric: &str, actual: f64, budget: f64, lower_is_better: bool| {
            checked += 1;
            let over = if lower_is_better {
                actual - budget
            } else {
                budget - actual
            };
            if over > 0.0 {
                violations.push(BudgetGate {
                    metric: metric.to_string(),
                    actual,
                    budget,
                    over_by: over,
                });
            }
        };

        // Synthetic / estimated CWV (e.g. HTTP-derived) is excluded from budgets.
        if let Some(v) = vitals.filter(|v| !v.synthetic) {
            if let Some(b) = self.max_lcp_ms {
                gate("lcp_ms", v.lcp_ms, b, true);
            }
            if let Some(b) = self.max_inp_ms {
                gate("inp_ms", v.inp_ms, b, true);
            }
            if let Some(b) = self.max_cls {
                gate("cls", v.cls, b, true);
            }
            if let Some(b) = self.max_tbt_ms {
                gate("tbt_ms", v.tbt_ms, b, true);
            }
            if let Some(b) = self.max_ttfb_ms {
                gate("ttfb_ms", v.ttfb_ms, b, true);
            }
        }

        if let Some(l) = load {
            if let Some(b) = self.max_p95_latency_ms {
                gate("p95_latency_ms", l.p95_latency_ms, b, true);
            }
            if let Some(b) = self.max_p99_latency_ms {
                gate("p99_latency_ms", l.p99_latency_ms, b, true);
            }
            if let Some(b) = self.max_error_rate {
                let error_rate = if l.total_requests > 0 {
                    l.failed_requests as f64 / l.total_requests as f64
                } else {
                    0.0
                };
                gate("error_rate", error_rate, b, true);
            }
            if let Some(b) = self.min_rps {
                gate("rps", l.rps, b, false);
            }
        }

        if let (Some(transfer), Some(max)) = (transfer_bytes, self.max_transfer_bytes) {
            gate("transfer_bytes", transfer as f64, max as f64, true);
        }
        if let (Some(transfer), Some(decoded), Some(max_ratio)) =
            (transfer_bytes, decoded_bytes, self.max_decoded_inflation_ratio)
        {
            if transfer > 0 {
                let ratio = decoded as f64 / transfer as f64;
                gate("decoded_inflation_ratio", ratio, max_ratio, true);
            }
        }
        if let (Some(media), Some(max)) = (lcp_media_bytes, self.max_lcp_media_bytes) {
            gate("lcp_media_bytes", media as f64, max as f64, true);
        }

        BudgetEvaluation {
            passed: violations.is_empty(),
            violations,
            checked,
        }
    }

    /// Evaluate measured [`LoadMetrics`] (and any embedded browser vitals)
    /// against the configured budgets, returning one human-readable message per
    /// violated gate. An empty result means every enforced budget passed.
    ///
    /// Delegates to [`Self::evaluate`] so SLA / load / PerfEngine cannot diverge.
    /// When [`LoadMetrics::soft_nav`] is present with navigations, soft-nav
    /// worst LCP / avg INP / total CLS are folded into the effective vitals
    /// (max with hard-nav values) so SPA route changes participate in gates.
    pub fn check(&self, metrics: &LoadMetrics) -> Vec<String> {
        // Drop synthetic browser vitals before folding — they must not gate budgets.
        let mut folded = metrics
            .browser_vitals
            .clone()
            .filter(|v| !v.synthetic);
        if let Some(soft) = metrics.soft_nav.as_ref() {
            if soft.navigations > 0 {
                let base = folded.get_or_insert_with(BrowserMetrics::default);
                base.lcp_ms = base.lcp_ms.max(soft.worst_lcp_ms);
                base.inp_ms = base.inp_ms.max(soft.avg_inp_ms);
                base.cls = base.cls.max(soft.total_cls);
                base.synthetic = false;
            }
        }
        let eval = self.evaluate(
            folded.as_ref(),
            Some(metrics),
            metrics.transfer_bytes,
            metrics.decoded_bytes,
            metrics.lcp_media_bytes,
        );
        eval.violations
            .iter()
            .map(|v| match v.metric.as_str() {
                "lcp_ms" => format!("LCP {:.2}ms exceeds budget {:.2}ms", v.actual, v.budget),
                "inp_ms" => format!("INP {:.2}ms exceeds budget {:.2}ms", v.actual, v.budget),
                "cls" => format!("CLS {:.4} exceeds budget {:.4}", v.actual, v.budget),
                "tbt_ms" => format!("TBT {:.2}ms exceeds budget {:.2}ms", v.actual, v.budget),
                "ttfb_ms" => format!("TTFB {:.2}ms exceeds budget {:.2}ms", v.actual, v.budget),
                "p95_latency_ms" => format!(
                    "p95 latency {:.2}ms exceeds budget {:.2}ms",
                    v.actual, v.budget
                ),
                "p99_latency_ms" => format!(
                    "p99 latency {:.2}ms exceeds budget {:.2}ms",
                    v.actual, v.budget
                ),
                "error_rate" => format!(
                    "error rate {:.4} exceeds budget {:.4}",
                    v.actual, v.budget
                ),
                "rps" => format!(
                    "throughput {:.2} rps below budget {:.2} rps",
                    v.actual, v.budget
                ),
                other => format!(
                    "{other} {:.4} exceeds budget {:.4} (over by {:.4})",
                    v.actual, v.budget, v.over_by
                ),
            })
            .collect()
    }
}

/// Canonical Thor Scenario Specification for Load Engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThorScenario {
    pub id: String,
    pub name: String,
    pub target_url: String,
    pub runner_type: RunnerType,
    pub vus: usize,
    pub duration_secs: u64,
    pub ramp_up_secs: u64,
    pub target_rps: Option<u32>,
    pub headers: HashMap<String, String>,
    pub steps: Vec<ScenarioStep>,
    #[serde(default)]
    pub budgets: PerformanceBudgets,
}

impl ThorScenario {
    pub fn new(name: impl Into<String>, target_url: impl Into<String>) -> Self {
        let name_str = name.into();
        let id = format!("scenario-{}", name_str.to_lowercase().replace(' ', "-"));
        Self {
            id,
            name: name_str,
            target_url: target_url.into(),
            runner_type: RunnerType::Protocol,
            vus: 1,
            duration_secs: 10,
            ramp_up_secs: 0,
            target_rps: None,
            headers: HashMap::new(),
            steps: Vec::new(),
            budgets: PerformanceBudgets::default(),
        }
    }

    pub fn with_budgets(mut self, budgets: PerformanceBudgets) -> Self {
        self.budgets = budgets;
        self
    }

    pub fn with_runner(mut self, runner_type: RunnerType) -> Self {
        self.runner_type = runner_type;
        self
    }

    pub fn with_vus(mut self, vus: usize) -> Self {
        self.vus = vus;
        self
    }

    pub fn with_duration(mut self, duration_secs: u64) -> Self {
        self.duration_secs = duration_secs;
        self
    }

    pub fn with_ramp_up(mut self, ramp_up_secs: u64) -> Self {
        self.ramp_up_secs = ramp_up_secs;
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn add_step(mut self, step: ScenarioStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    use crate::load::metrics::{BrowserMetrics, LoadMetrics};

    fn load_metrics(p95: f64, p99: f64, total: usize, failed: usize, rps: f64) -> LoadMetrics {
        LoadMetrics {
            total_requests: total,
            successful_requests: total - failed,
            failed_requests: failed,
            total_bytes: 0,
            duration_secs: 1.0,
            rps,
            min_latency_ms: 0.0,
            max_latency_ms: p99,
            avg_latency_ms: p95,
            p50_latency_ms: p95 / 2.0,
            p90_latency_ms: p95,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            status_codes: HashMap::new(),
            browser_vitals: None,
            transfer_bytes: None,
            decoded_bytes: None,
            lcp_media_bytes: None,
            soft_nav: None,
        }
    }

    #[test]
    fn empty_budgets_never_violate() {
        let budgets = PerformanceBudgets::default();
        let metrics = load_metrics(9999.0, 99999.0, 10, 10, 0.0);
        assert!(budgets.check(&metrics).is_empty());
    }

    #[test]
    fn latency_throughput_and_error_gates_fire() {
        let budgets = PerformanceBudgets {
            max_p95_latency_ms: Some(500.0),
            max_p99_latency_ms: Some(1000.0),
            max_error_rate: Some(0.01),
            min_rps: Some(50.0),
            ..PerformanceBudgets::default()
        };
        // Within budget: 1 failure in 1000 = 0.001 error rate, fast, high rps.
        let ok = load_metrics(300.0, 800.0, 1000, 1, 120.0);
        assert!(budgets.check(&ok).is_empty(), "{:?}", budgets.check(&ok));

        // Every gate breached at once.
        let bad = load_metrics(900.0, 1500.0, 100, 50, 10.0);
        let violations = budgets.check(&bad);
        assert_eq!(violations.len(), 4, "{violations:?}");
        assert!(violations.iter().any(|v| v.contains("p95")));
        assert!(violations.iter().any(|v| v.contains("p99")));
        assert!(violations.iter().any(|v| v.contains("error rate")));
        assert!(violations.iter().any(|v| v.contains("throughput")));
    }

    #[test]
    fn vitals_gates_only_apply_when_measured() {
        let budgets = PerformanceBudgets::core_web_vitals();
        // Protocol-only run (no vitals) is never failed for missing LCP.
        let protocol_only = load_metrics(10.0, 20.0, 100, 0, 100.0);
        assert!(budgets.check(&protocol_only).is_empty());

        // A browser run with poor vitals trips the gate.
        let mut browser_run = load_metrics(10.0, 20.0, 100, 0, 100.0);
        browser_run.browser_vitals = Some(BrowserMetrics {
            lcp_ms: 4000.0,
            inp_ms: 350.0,
            cls: 0.4,
            ..Default::default()
        });
        let violations = budgets.check(&browser_run);
        assert!(violations.iter().any(|v| v.contains("LCP")));
        assert!(violations.iter().any(|v| v.contains("INP")));
        assert!(violations.iter().any(|v| v.contains("CLS")));
    }

    #[test]
    fn synthetic_vitals_excluded_from_budget_gates() {
        let budgets = PerformanceBudgets::core_web_vitals();
        let mut metrics = load_metrics(10.0, 20.0, 100, 0, 100.0);
        // Terrible numbers — but synthetic must not trip CWV gates.
        metrics.browser_vitals = Some(BrowserMetrics {
            lcp_ms: 9000.0,
            inp_ms: 900.0,
            cls: 0.9,
            synthetic: true,
            ..Default::default()
        });
        assert!(
            budgets.check(&metrics).is_empty(),
            "synthetic CWV must be excluded from budgets"
        );
    }

    #[test]
    fn check_uses_measured_transfer_and_decoded_bytes() {
        let budgets = PerformanceBudgets {
            max_transfer_bytes: Some(1_000),
            max_decoded_inflation_ratio: Some(2.0),
            ..PerformanceBudgets::default()
        };
        let mut metrics = load_metrics(10.0, 20.0, 10, 0, 10.0);
        // Without measured bytes, transfer/inflation gates stay silent.
        assert!(budgets.check(&metrics).is_empty());

        metrics.transfer_bytes = Some(5_000);
        metrics.decoded_bytes = Some(20_000); // ratio 4.0 > 2.0
        let violations = budgets.check(&metrics);
        assert!(
            violations.iter().any(|v| v.contains("transfer_bytes")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("decoded_inflation_ratio")),
            "{violations:?}"
        );
    }
}
