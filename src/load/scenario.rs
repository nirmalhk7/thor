use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
