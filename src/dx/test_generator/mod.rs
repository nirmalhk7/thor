mod artifacts;
mod http;
mod playwright;
mod policies;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Export target format for Universal 'Promote to Test'
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    PlaywrightTs,
    K6Js,
    Curl,
    /// Deterministic replay/eval harness (Node) — replays recorded traces as
    /// assertable eval cases (used by AI eval-replay & determinism harness).
    EvalHarness,
    /// Portable CI bundle (GitHub Actions workflow) that runs the exported test.
    CiBundle,
    /// Structured security/QA finding document (Markdown) promoted from a session.
    Finding,
    ThorScenario,
    ReqwestRust,
    GoHttp,
    PythonHttpx,
}

/// Recorded network trace entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTrace {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub status: u16,
}

/// Resilient locator candidate for a DOM element, in Playwright priority order.
/// A `None` field is not available for this element. `to_playwright` emits the
/// highest-priority available strategy (test-id > role > label > text > css).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocatorCandidate {
    pub test_id: Option<String>,
    pub role: Option<String>,
    /// Accessible name paired with `role`.
    pub name: Option<String>,
    pub label: Option<String>,
    pub text: Option<String>,
    pub css: Option<String>,
}

impl LocatorCandidate {
    /// Infer candidate locators from a raw CSS selector, extracting a
    /// `data-testid` when present and otherwise keeping the raw selector.
    pub fn from_selector(selector: &str) -> Self {
        let mut c = LocatorCandidate::default();
        if let Some(id) = extract_attr(selector, "data-testid")
            .or_else(|| extract_attr(selector, "data-test-id"))
            .or_else(|| extract_attr(selector, "data-test"))
        {
            c.test_id = Some(id);
        }
        c.css = Some(selector.to_string());
        c
    }

    /// Emit the best available Playwright locator expression, or `None` when the
    /// candidate carries no usable strategy.
    pub fn to_playwright(&self) -> Option<String> {
        if let Some(id) = &self.test_id {
            return Some(format!("page.getByTestId('{}')", js_str(id)));
        }
        if let Some(role) = &self.role {
            return Some(match &self.name {
                Some(name) => format!(
                    "page.getByRole('{}', {{ name: '{}' }})",
                    js_str(role),
                    js_str(name)
                ),
                None => format!("page.getByRole('{}')", js_str(role)),
            });
        }
        if let Some(label) = &self.label {
            return Some(format!("page.getByLabel('{}')", js_str(label)));
        }
        if let Some(text) = &self.text {
            return Some(format!("page.getByText('{}')", js_str(text)));
        }
        self.css
            .as_ref()
            .map(|css| format!("page.locator('{}')", js_str(css)))
    }
}

/// Recorded DOM interaction step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomStep {
    pub action: String,
    pub target: String,
    pub value: Option<String>,
    /// Resilient locator candidate; when absent it is inferred from `target`.
    #[serde(default)]
    pub locator: Option<LocatorCandidate>,
}

impl DomStep {
    pub fn new(action: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            target: target.into(),
            value: None,
            locator: None,
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_locator(mut self, locator: LocatorCandidate) -> Self {
        self.locator = Some(locator);
        self
    }

    /// Resolve the Playwright locator expression for this step, inferring from
    /// the raw `target` selector when no explicit candidate was recorded.
    pub fn playwright_locator(&self) -> String {
        self.locator
            .as_ref()
            .and_then(|l| l.to_playwright())
            .or_else(|| LocatorCandidate::from_selector(&self.target).to_playwright())
            .unwrap_or_else(|| format!("page.locator('{}')", js_str(&self.target)))
    }
}

fn js_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn extract_attr(selector: &str, attr: &str) -> Option<String> {
    let needle = format!("{}=", attr);
    let start = selector.find(&needle)? + needle.len();
    let rest = &selector[start..];
    let rest = rest.trim_start_matches(['"', '\'']);
    let end = rest.find(['"', '\'', ']']).unwrap_or(rest.len());
    let val = rest[..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

/// Recorded session containing both network traces and DOM interaction steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSession {
    pub name: String,
    pub target_url: String,
    pub cookies: HashMap<String, String>,
    pub traces: Vec<NetworkTrace>,
    pub dom_steps: Vec<DomStep>,
}

impl RecordedSession {
    pub fn new(name: impl Into<String>, target_url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            target_url: target_url.into(),
            cookies: HashMap::new(),
            traces: Vec::new(),
            dom_steps: Vec::new(),
        }
    }

    pub fn add_cookie(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.cookies.insert(key.into(), val.into());
    }

    pub fn add_trace(&mut self, trace: NetworkTrace) {
        self.traces.push(trace);
    }

    pub fn add_dom_step(&mut self, step: DomStep) {
        self.dom_steps.push(step);
    }
}

/// Universal Test Generator for Thor ('Promote to Test')
pub struct TestGenerator;

impl TestGenerator {
    /// Convert recorded session into target format
    pub fn promote_to_test(session: &RecordedSession, format: ExportFormat) -> String {
        match format {
            ExportFormat::PlaywrightTs => Self::export_playwright_ts(session),
            ExportFormat::K6Js => Self::export_k6_js(session),
            ExportFormat::Curl => Self::export_curl(session),
            ExportFormat::EvalHarness => Self::export_eval_harness(session),
            ExportFormat::CiBundle => Self::export_ci_bundle(session),
            ExportFormat::Finding => Self::export_finding(session),
            ExportFormat::ThorScenario => Self::export_thor_scenario(session),
            ExportFormat::ReqwestRust => Self::export_reqwest_rust(session),
            ExportFormat::GoHttp => Self::export_go_http(session),
            ExportFormat::PythonHttpx => Self::export_python_httpx(session),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promote_to_test_export() {
        let mut session = RecordedSession::new("Login Flow", "https://api.example.com");
        session.add_dom_step(
            DomStep::new("fill", "#username")
                .with_value("thor_user")
                .with_locator(LocatorCandidate {
                    test_id: Some("username".to_string()),
                    ..Default::default()
                }),
        );
        session.add_dom_step(DomStep::new("click", "[data-testid=\"submit\"]"));
        session.add_trace(NetworkTrace {
            method: "POST".to_string(),
            url: "https://api.example.com/v1/auth".to_string(),
            headers: [("Content-Type".to_string(), "application/json".to_string())]
                .into_iter()
                .collect(),
            body: Some(r#"{"user":"thor"}"#.to_string()),
            status: 200,
        });

        let ts = TestGenerator::promote_to_test(&session, ExportFormat::PlaywrightTs);
        assert!(ts.contains("page.getByTestId('username').fill('thor_user')"));
        assert!(ts.contains("await expect(page.getByTestId('username')).toBeVisible();"));
        // Click step falls back to inferring test-id from the selector.
        assert!(ts.contains("page.getByTestId('submit').click()"));
        assert!(!ts.contains("waitForTimeout"));
        assert!(ts.contains("request.post"));

        let k6 = TestGenerator::promote_to_test(&session, ExportFormat::K6Js);
        assert!(k6.contains("http.request('POST'"));

        let curl = TestGenerator::promote_to_test(&session, ExportFormat::Curl);
        assert!(curl.contains("curl -i -X POST \"https://api.example.com/v1/auth\""));

        let thor_scen = TestGenerator::promote_to_test(&session, ExportFormat::ThorScenario);
        assert!(thor_scen.contains("Login Flow"));
        assert!(thor_scen.contains("https://api.example.com"));
        assert!(thor_scen.contains("#username"));
    }

    #[test]
    fn test_new_export_formats() {
        let mut session = RecordedSession::new("Checkout", "https://shop.example.com");
        session.add_trace(NetworkTrace {
            method: "GET".to_string(),
            url: "https://shop.example.com/cart".to_string(),
            headers: HashMap::new(),
            body: None,
            status: 500,
        });

        let harness = TestGenerator::promote_to_test(&session, ExportFormat::EvalHarness);
        assert!(harness.contains("const CASES = ["));
        assert!(harness.contains("expectStatus: 500"));

        let ci = TestGenerator::promote_to_test(&session, ExportFormat::CiBundle);
        assert!(ci.contains("playwright install"));
        assert!(ci.contains("name: thor-e2e-checkout"));

        let finding = TestGenerator::promote_to_test(&session, ExportFormat::Finding);
        assert!(finding.contains("# Finding: Checkout"));
        assert!(finding.contains("→ 500"));
    }

    #[test]
    fn test_oracle_quarantine_a11y_export_helpers() {
        let base = "import { test, expect } from '@playwright/test';\ntest('t', async ({ page }) => {\n  await page.goto('/');\n});\n";
        let with_oracles =
            TestGenerator::append_oracles(base, &["expect(response.status()).toBe(200)".into()]);
        assert!(with_oracles.contains("Thor synthesized oracles"));
        assert!(with_oracles.contains("toBe(200)"));

        let yaml =
            TestGenerator::quarantine_yaml(&[("flaky".into(), "skip".into(), "rate high".into())]);
        assert!(yaml.contains("quarantine:"));
        assert!(yaml.contains("action: skip"));

        let titles = TestGenerator::quarantined_titles_from_yaml(&yaml);
        assert_eq!(titles, vec!["flaky".to_string()]);
        let filter = TestGenerator::ci_quarantine_filter(&titles);
        assert!(filter.contains("--grep-invert"));
        assert!(filter.contains("flaky"));

        let a11y = TestGenerator::a11y_spec_scaffold("home", &["label".into()]);
        assert!(a11y.contains("AxeBuilder"));
        assert!(a11y.contains("label"));
    }
}
