use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::load::metrics::LoadMetrics;

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
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == PerformanceBudgets::default()
    }

    /// Evaluate measured [`LoadMetrics`] (and any embedded browser vitals)
    /// against the configured budgets, returning one human-readable message per
    /// violated gate. An empty result means every enforced budget passed.
    ///
    /// Budget fields left as `None` are not enforced. Web-vitals gates are only
    /// checked when the run actually produced `browser_vitals`, so a
    /// protocol-only run is never failed for a missing LCP measurement.
    pub fn check(&self, metrics: &LoadMetrics) -> Vec<String> {
        let mut violations = Vec::new();

        if let Some(max) = self.max_p95_latency_ms {
            if metrics.p95_latency_ms > max {
                violations.push(format!(
                    "p95 latency {:.2}ms exceeds budget {:.2}ms",
                    metrics.p95_latency_ms, max
                ));
            }
        }
        if let Some(max) = self.max_p99_latency_ms {
            if metrics.p99_latency_ms > max {
                violations.push(format!(
                    "p99 latency {:.2}ms exceeds budget {:.2}ms",
                    metrics.p99_latency_ms, max
                ));
            }
        }
        if let Some(max) = self.max_error_rate {
            let error_rate = if metrics.total_requests > 0 {
                metrics.failed_requests as f64 / metrics.total_requests as f64
            } else {
                0.0
            };
            if error_rate > max {
                violations.push(format!(
                    "error rate {:.4} exceeds budget {:.4}",
                    error_rate, max
                ));
            }
        }
        if let Some(min) = self.min_rps {
            if metrics.rps < min {
                violations.push(format!(
                    "throughput {:.2} rps below budget {:.2} rps",
                    metrics.rps, min
                ));
            }
        }

        if let Some(vitals) = &metrics.browser_vitals {
            if let Some(max) = self.max_lcp_ms {
                if vitals.lcp_ms > max {
                    violations.push(format!(
                        "LCP {:.2}ms exceeds budget {:.2}ms",
                        vitals.lcp_ms, max
                    ));
                }
            }
            if let Some(max) = self.max_inp_ms {
                if vitals.inp_ms > max {
                    violations.push(format!(
                        "INP {:.2}ms exceeds budget {:.2}ms",
                        vitals.inp_ms, max
                    ));
                }
            }
            if let Some(max) = self.max_cls {
                if vitals.cls > max {
                    violations.push(format!("CLS {:.4} exceeds budget {:.4}", vitals.cls, max));
                }
            }
            if let Some(max) = self.max_tbt_ms {
                if vitals.tbt_ms > max {
                    violations.push(format!(
                        "TBT {:.2}ms exceeds budget {:.2}ms",
                        vitals.tbt_ms, max
                    ));
                }
            }
            if let Some(max) = self.max_ttfb_ms {
                if vitals.ttfb_ms > max {
                    violations.push(format!(
                        "TTFB {:.2}ms exceeds budget {:.2}ms",
                        vitals.ttfb_ms, max
                    ));
                }
            }
        }

        violations
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
}
