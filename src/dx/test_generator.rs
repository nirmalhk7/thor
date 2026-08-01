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

    /// Append synthesized oracle assertion lines to a Playwright export.
    pub fn append_oracles(playwright_ts: &str, oracles: &[String]) -> String {
        if oracles.is_empty() {
            return playwright_ts.to_string();
        }
        let mut out = playwright_ts.trim_end().to_string();
        // Insert before the final closing `});` of the test body when present.
        if let Some(idx) = out.rfind("});") {
            let mut block = String::from("\n  // Thor synthesized oracles\n");
            for o in oracles {
                block.push_str("  ");
                block.push_str(o);
                if !o.ends_with(';') {
                    block.push(';');
                }
                block.push('\n');
            }
            out.insert_str(idx, &block);
            out
        } else {
            out.push_str("\n// Thor synthesized oracles\n");
            for o in oracles {
                out.push_str(o);
                out.push('\n');
            }
            out
        }
    }

    /// Emit a quarantine.yaml snippet for CI skip/quarantine policies.
    pub fn quarantine_yaml(entries: &[(String, String, String)]) -> String {
        let mut out = String::from("quarantine:\n");
        for (test, action, reason) in entries {
            out.push_str(&format!(
                "  - test: \"{}\"\n    action: {}\n    reason: \"{}\"\n",
                test.replace('"', "\\\""),
                action,
                reason.replace('"', "\\\"")
            ));
        }
        out
    }

    /// CI filter snippet that skips quarantined Playwright titles via
    /// `--grep-invert` / `testIgnore` — consumers paste into workflow YAML.
    /// Always non-empty (stub comment block when quarantine list is empty).
    pub fn ci_quarantine_filter(quarantined_tests: &[String]) -> String {
        if quarantined_tests.is_empty() {
            return String::from(
                "# Thor CI quarantine filter (stub — no quarantined tests)\n# npx playwright test\n",
            );
        }
        let pattern = quarantined_tests
            .iter()
            .map(|t| regex_escape(t))
            .collect::<Vec<_>>()
            .join("|");
        format!(
            concat!(
                "# Thor CI quarantine filter (generated)\n",
                "# Quarantined: {count} test(s)\n",
                "npx playwright test --grep-invert '{pattern}'\n",
                "# Or in playwright.config.ts:\n",
                "#   grepInvert: /{pattern}/\n",
            ),
            count = quarantined_tests.len(),
            pattern = pattern,
        )
    }

    /// Parse test titles marked `action: quarantine` or `action: skip` from a
    /// quarantine.yaml body (best-effort line scanner).
    pub fn quarantined_titles_from_yaml(yaml: &str) -> Vec<String> {
        let mut titles = Vec::new();
        let mut pending: Option<String> = None;
        for line in yaml.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("- test:") {
                let name = rest
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                pending = Some(name);
            } else if let Some(rest) = trimmed.strip_prefix("action:") {
                let action = rest.trim();
                if matches!(action, "quarantine" | "skip") {
                    if let Some(t) = pending.take() {
                        titles.push(t);
                    }
                } else {
                    pending = None;
                }
            }
        }
        titles
    }

    /// Emit an axe-style Playwright a11y assertion scaffold for a page name.
    pub fn a11y_spec_scaffold(page_name: &str, rule_ids: &[String]) -> String {
        let mut out = String::new();
        out.push_str("import { test, expect } from '@playwright/test';\n");
        out.push_str("import AxeBuilder from '@axe-core/playwright';\n\n");
        out.push_str(&format!(
            "test('a11y: {}', async ({{ page }}) => {{\n",
            page_name.replace('\'', "\\'")
        ));
        out.push_str("  const results = await new AxeBuilder({ page }).analyze();\n");
        out.push_str("  expect(results.violations).toEqual([]);\n");
        for rule in rule_ids {
            out.push_str(&format!(
                "  expect(results.violations.find(v => v.id === '{}')).toBeUndefined();\n",
                rule.replace('\'', "\\'")
            ));
        }
        out.push_str("});\n");
        out
    }

    /// Export session to Playwright TypeScript test
    pub fn export_playwright_ts(session: &RecordedSession) -> String {
        let mut out = String::new();
        out.push_str("// Generated by Thor Universal 'Promote to Test'\n");
        out.push_str("import { test, expect } from '@playwright/test';\n\n");
        out.push_str(&format!(
            "test('{} - Automated End-to-End Test', async ({{ page, context, request }}) => {{\n",
            session.name
        ));

        if !session.cookies.is_empty() {
            out.push_str("  await context.addCookies([\n");
            for (k, v) in &session.cookies {
                out.push_str(&format!(
                    "    {{ name: '{}', value: '{}', url: '{}' }},\n",
                    k, v, session.target_url
                ));
            }
            out.push_str("  ]);\n");
        }

        out.push_str(&format!("  await page.goto('{}');\n", session.target_url));

        // Export DOM steps using resilient role/test-id locators & web-first assertions
        for step in &session.dom_steps {
            match step.action.to_lowercase().as_str() {
                "click" => {
                    let loc = step.playwright_locator();
                    out.push_str(&format!("  await expect({}).toBeVisible();\n", loc));
                    out.push_str(&format!("  await {}.click();\n", loc));
                }
                "fill" => {
                    let loc = step.playwright_locator();
                    let val = js_str(step.value.as_deref().unwrap_or(""));
                    out.push_str(&format!("  await expect({}).toBeVisible();\n", loc));
                    out.push_str(&format!("  await {}.fill('{}');\n", loc, val));
                }
                "assert_visible" => {
                    out.push_str(&format!("  await expect(page.locator('{}')).toBeVisible();\n", step.target));
                }
                "assert_text" => {
                    let val = step.value.as_deref().unwrap_or("");
                    out.push_str(&format!("  await expect(page.locator('{}')).toHaveText('{}');\n", step.target, val));
                }
                "drag" => {
                    let dest = step.value.as_deref().unwrap_or("body");
                    out.push_str(&format!("  await page.locator('{}').dragTo(page.locator('{}'));\n", step.target, dest));
                }
                "scroll" => {
                    let dy: i32 = step.value.as_deref().and_then(|s| s.parse().ok()).unwrap_or(500);
                    out.push_str(&format!("  await page.mouse.wheel(0, {});\n", dy));
                }
                "iframe_click" => {
                    let el = step.value.as_deref().unwrap_or("button");
                    out.push_str(&format!("  await page.frameLocator('{}').locator('{}').click();\n", step.target, el));
                }
                "navigate" => {
                    out.push_str(&format!("  await page.goto('{}');\n", js_str(&step.target)));
                }
                "wait" => {
                    // Replace brittle fixed timeouts with web-first waits: wait on
                    // the network to settle, or on the target locator to appear.
                    if step.target.trim().parse::<u64>().is_ok() {
                        out.push_str("  await page.waitForLoadState('networkidle');\n");
                    } else {
                        let loc = step.playwright_locator();
                        out.push_str(&format!("  await expect({}).toBeVisible();\n", loc));
                    }
                }
                "expect" | "assert" => {
                    let loc = step.playwright_locator();
                    match step.value.as_deref() {
                        Some(text) if !text.is_empty() => out.push_str(&format!(
                            "  await expect({}).toContainText('{}');\n",
                            loc,
                            js_str(text)
                        )),
                        _ => out.push_str(&format!("  await expect({}).toBeVisible();\n", loc)),
                    }
                }
                _ => {
                    out.push_str(&format!(
                        "  // Action: {} on {}\n",
                        step.action, step.target
                    ));
                }
            }
        }

        // Export Network requests using Playwright request API
        for trace in &session.traces {
            let method = trace.method.to_lowercase();
            out.push_str(&format!(
                "  const res_{} = await request.{}(\"{}\", {{\n",
                trace.method.to_lowercase(),
                method,
                trace.url
            ));
            if !trace.headers.is_empty() {
                out.push_str("    headers: {\n");
                for (k, v) in &trace.headers {
                    out.push_str(&format!("      '{}': '{}',\n", k, v.replace('\'', "\\'")));
                }
                out.push_str("    },\n");
            }
            if let Some(ref b) = trace.body {
                let escaped_b = b.replace('`', "\\`").replace('$', "\\$");
                out.push_str(&format!("    data: `{}`, \n", escaped_b));
            }
            out.push_str("  });\n");
            out.push_str(&format!(
                "  expect(res_{}.status()).toBe({});\n",
                trace.method.to_lowercase(),
                if trace.status > 0 { trace.status } else { 200 }
            ));
        }

        out.push_str("});\n");
        out
    }

    /// Export session to k6 JavaScript load test script
    pub fn export_k6_js(session: &RecordedSession) -> String {
        let mut out = String::new();
        out.push_str("// Generated by Thor Universal 'Promote to Test' for k6\n");
        out.push_str("import http from 'k6/http';\n");
        out.push_str("import { check, sleep } from 'k6';\n\n");
        out.push_str("export const options = {\n");
        out.push_str("  vus: 10,\n");
        out.push_str("  duration: '30s',\n");
        out.push_str("};\n\n");
        out.push_str("export default function () {\n");
        out.push_str(&format!(
            "  let res = http.get('{}');\n",
            session.target_url
        ));
        out.push_str("  check(res, { 'status is 200': (r) => r.status === 200 });\n");
        out.push_str("  sleep(1);\n\n");

        for (idx, trace) in session.traces.iter().enumerate() {
            let var_name = format!("res{}", idx + 1);
            let method = trace.method.to_uppercase();
            let mut params = String::from("    headers: {\n");
            for (k, v) in &trace.headers {
                params.push_str(&format!("      '{}': '{}',\n", k, v.replace('\'', "\\'")));
            }
            params.push_str("    },\n");

            if method == "POST" || method == "PUT" || method == "PATCH" {
                let payload = trace.body.as_deref().unwrap_or("");
                let escaped_payload = payload.replace('`', "\\`").replace('$', "\\$");
                out.push_str(&format!(
                    "  let {} = http.request('{}', '{}', `{}`, {{\n{}\n  }});\n",
                    var_name,
                    method,
                    trace.url,
                    escaped_payload,
                    params
                ));
            } else {
                out.push_str(&format!(
                    "  let {} = http.request('{}', '{}', null, {{\n{}\n  }});\n",
                    var_name, method, trace.url, params
                ));
            }

            let expected_status = if trace.status > 0 { trace.status } else { 200 };
            out.push_str(&format!(
                "  check({}, {{ 'status is {}': (r) => r.status === {} }});\n",
                var_name, expected_status, expected_status
            ));
            out.push_str("  sleep(0.5);\n\n");
        }

        out.push_str("}\n");
        out
    }

    /// Export session to cURL commands
    pub fn export_curl(session: &RecordedSession) -> String {
        let mut out = String::new();
        out.push_str("#!/usr/bin/env bash\n");
        out.push_str("# Generated by Thor Universal 'Promote to Test'\n\n");

        if session.traces.is_empty() {
            out.push_str(&format!("curl -i -X GET \"{}\"\n", session.target_url));
            return out;
        }

        for trace in &session.traces {
            out.push_str(&format!(
                "curl -i -X {} \"{}\"",
                trace.method.to_uppercase(),
                trace.url
            ));
            for (k, v) in &trace.headers {
                out.push_str(&format!(" \\\n  -H \"{}: {}\"", k, v));
            }
            if let Some(ref body) = trace.body {
                let escaped_body = body.replace('\'', "'\\''");
                out.push_str(&format!(" \\\n  -d '{}'", escaped_body));
            }
            out.push_str("\n\n");
        }

        out
    }

    /// Export a deterministic replay/eval harness (Node) that replays each
    /// recorded trace and asserts the recorded status — the backbone of
    /// AI eval-replay and determinism checks.
    pub fn export_eval_harness(session: &RecordedSession) -> String {
        let mut out = String::new();
        out.push_str(
            "// Generated by Thor Universal 'Promote to Test' — Deterministic Eval Harness\n",
        );
        out.push_str("// Run: node eval_harness.mjs (Node 18+ with global fetch)\n\n");
        out.push_str(&format!(
            "const TARGET = {};\n",
            serde_json::to_string(&session.target_url).unwrap_or_else(|_| "\"\"".into())
        ));
        out.push_str("const CASES = [\n");
        for (idx, trace) in session.traces.iter().enumerate() {
            let headers = serde_json::to_string(&trace.headers).unwrap_or_else(|_| "{}".into());
            let body = trace
                .body
                .as_ref()
                .map(|b| serde_json::to_string(b).unwrap_or_else(|_| "null".into()))
                .unwrap_or_else(|| "null".into());
            let expected = if trace.status > 0 { trace.status } else { 200 };
            out.push_str(&format!(
                "  {{ id: {}, method: {}, url: {}, headers: {}, body: {}, expectStatus: {} }},\n",
                idx + 1,
                serde_json::to_string(&trace.method).unwrap_or_else(|_| "\"GET\"".into()),
                serde_json::to_string(&trace.url).unwrap_or_else(|_| "\"\"".into()),
                headers,
                body,
                expected
            ));
        }
        out.push_str("];\n\n");
        out.push_str("let failures = 0;\n");
        out.push_str("for (const c of CASES) {\n");
        out.push_str("  const opts = { method: c.method, headers: c.headers };\n");
        out.push_str("  if (c.body != null) opts.body = c.body;\n");
        out.push_str("  const res = await fetch(c.url, opts);\n");
        out.push_str("  const ok = res.status === c.expectStatus;\n");
        out.push_str("  if (!ok) failures++;\n");
        out.push_str("  console.log(`[case ${c.id}] ${c.method} ${c.url} -> ${res.status} (expect ${c.expectStatus}) ${ok ? 'PASS' : 'FAIL'}`);\n");
        out.push_str("}\n");
        out.push_str(
            "console.log(`\\n${CASES.length - failures}/${CASES.length} cases passed`);\n",
        );
        out.push_str("process.exit(failures === 0 ? 0 : 1);\n");
        out
    }

    /// Export a portable GitHub Actions CI bundle that installs Playwright and
    /// runs the generated end-to-end spec.
    pub fn export_ci_bundle(session: &RecordedSession) -> String {
        let safe_name = session
            .name
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "-");
        let mut out = String::new();
        out.push_str("# Generated by Thor Universal 'Promote to Test' — CI Bundle\n");
        out.push_str(&format!("name: thor-e2e-{}\n\n", safe_name));
        out.push_str("on:\n  push:\n  workflow_dispatch:\n\n");
        out.push_str("jobs:\n");
        out.push_str("  e2e:\n");
        out.push_str("    runs-on: ubuntu-latest\n");
        out.push_str("    steps:\n");
        out.push_str("      - uses: actions/checkout@v4\n");
        out.push_str("      - uses: actions/setup-node@v4\n");
        out.push_str("        with:\n          node-version: '20'\n");
        out.push_str("      - run: npm ci || npm install\n");
        out.push_str("      - run: npx playwright install --with-deps chromium\n");
        out.push_str(&format!(
            "      - run: npx playwright test # target: {}\n",
            session.target_url
        ));
        out
    }

    /// Export a structured security/QA finding document (Markdown) summarizing a
    /// recorded session — used by 'Promote to Finding'.
    pub fn export_finding(session: &RecordedSession) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Finding: {}\n\n", session.name));
        out.push_str(&format!("- **Target:** {}\n", session.target_url));
        out.push_str(&format!(
            "- **Recorded requests:** {}\n",
            session.traces.len()
        ));
        out.push_str(&format!(
            "- **DOM interactions:** {}\n\n",
            session.dom_steps.len()
        ));

        let error_traces: Vec<&NetworkTrace> =
            session.traces.iter().filter(|t| t.status >= 400).collect();
        out.push_str("## Summary\n\n");
        if error_traces.is_empty() {
            out.push_str("No 4xx/5xx responses were observed in this session.\n\n");
        } else {
            out.push_str(&format!(
                "{} request(s) returned error status codes:\n\n",
                error_traces.len()
            ));
            for t in &error_traces {
                out.push_str(&format!("- `{} {}` → {}\n", t.method, t.url, t.status));
            }
            out.push('\n');
        }

        out.push_str("## Reproduction\n\n");
        out.push_str("```bash\n");
        out.push_str(&Self::export_curl(session));
        out.push_str("```\n");
        out
    }

    /// Export session to Rust reqwest client code
    pub fn export_reqwest_rust(session: &RecordedSession) -> String {
        let mut out = String::new();
        out.push_str("// Generated by Thor Universal 'Promote to Test' for Rust reqwest\n");
        out.push_str("use reqwest::header::{HeaderMap, HeaderName, HeaderValue};\n\n");
        out.push_str("#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
        out.push_str("    let client = reqwest::Client::new();\n\n");

        for trace in &session.traces {
            out.push_str(&format!("    // {} {}\n", trace.method.to_uppercase(), trace.url));
            out.push_str("    let mut headers = HeaderMap::new();\n");
            for (k, v) in &trace.headers {
                out.push_str(&format!(
                    "    headers.insert(HeaderName::from_static(\"{}\"), HeaderValue::from_str(\"{}\")?);\n",
                    k.to_lowercase(), v.replace('"', "\\\"")
                ));
            }
            let method = trace.method.to_lowercase();
            out.push_str(&format!("    let res = client.{}(\"{}\")\n        .headers(headers)\n", method, trace.url));
            if let Some(ref body) = trace.body {
                out.push_str(&format!("        .body(r#\"{}\"#)\n", body));
            }
            out.push_str("        .send()\n        .await?;\n");
            out.push_str("    println!(\"Status: {}\", res.status());\n\n");
        }

        out.push_str("    Ok(())\n}\n");
        out
    }

    /// Export session to Go net/http client code
    pub fn export_go_http(session: &RecordedSession) -> String {
        let mut out = String::new();
        out.push_str("// Generated by Thor Universal 'Promote to Test' for Go net/http\n");
        out.push_str("package main\n\nimport (\n\t\"fmt\"\n\t\"net/http\"\n\t\"strings\"\n)\n\n");
        out.push_str("func main() {\n\tclient := &http.Client{}\n\n");

        for trace in &session.traces {
            let body_expr = if let Some(ref body) = trace.body {
                format!("strings.NewReader(`{}`)", body)
            } else {
                "nil".to_string()
            };

            out.push_str(&format!(
                "\treq, err := http.NewRequest(\"{}\", \"{}\", {})\n\tif err != nil {{ panic(err) }}\n",
                trace.method.to_uppercase(), trace.url, body_expr
            ));

            for (k, v) in &trace.headers {
                out.push_str(&format!("\treq.Header.Set(\"{}\", \"{}\")\n", k, v.replace('"', "\\\"")));
            }

            out.push_str("\tres, err := client.Do(req)\n\tif err != nil {{ panic(err) }}\n");
            out.push_str("\tfmt.Println(\"Status:\", res.Status)\n\tres.Body.Close()\n\n");
        }

        out.push_str("}\n");
        out
    }

    /// Export session to Python httpx code
    pub fn export_python_httpx(session: &RecordedSession) -> String {
        let mut out = String::new();
        out.push_str("# Generated by Thor Universal 'Promote to Test' for Python httpx\n");
        out.push_str("import httpx\n\n");
        out.push_str("def main():\n    with httpx.Client() as client:\n");

        for trace in &session.traces {
            let method = trace.method.to_lowercase();
            let body_arg = if let Some(ref body) = trace.body {
                format!(", content='{}'", body.replace('\'', "\\'"))
            } else {
                "".to_string()
            };

            out.push_str(&format!("        res = client.{}(\"{}\"{})\n", method, trace.url, body_arg));
            out.push_str("        print(f\"Status: {res.status_code}\")\n\n");
        }

        out.push_str("if __name__ == '__main__':\n    main()\n");
        out
    }

    /// Export session to ThorScenario JSON format
    pub fn export_thor_scenario(session: &RecordedSession) -> String {
        use crate::load::scenario::{RunnerType, ScenarioStep, ThorScenario};

        let mut scenario = ThorScenario::new(&session.name, &session.target_url)
            .with_runner(if !session.dom_steps.is_empty() {
                RunnerType::Browser
            } else {
                RunnerType::Protocol
            });

        for (k, v) in &session.cookies {
            scenario = scenario.with_header("Cookie", format!("{k}={v}"));
        }

        for step in &session.dom_steps {
            match step.action.to_lowercase().as_str() {
                "click" => {
                    scenario = scenario.add_step(ScenarioStep::Click {
                        selector: step.target.clone(),
                    });
                }
                "fill" => {
                    scenario = scenario.add_step(ScenarioStep::Fill {
                        selector: step.target.clone(),
                        value: step.value.clone().unwrap_or_default(),
                    });
                }
                "navigate" => {
                    scenario = scenario.add_step(ScenarioStep::Navigate {
                        url: step.target.clone(),
                    });
                }
                "wait" => {
                    let ms: u64 = step.target.parse().unwrap_or(1000);
                    scenario = scenario.add_step(ScenarioStep::Wait { duration_ms: ms });
                }
                _ => {}
            }
        }

        for trace in &session.traces {
            scenario = scenario.add_step(ScenarioStep::HttpRequest {
                method: trace.method.clone(),
                url: trace.url.clone(),
                headers: trace.headers.clone(),
                body: trace.body.clone(),
            });
        }

        scenario.to_json().unwrap_or_default()
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
        let with_oracles = TestGenerator::append_oracles(
            base,
            &["expect(response.status()).toBe(200)".into()],
        );
        assert!(with_oracles.contains("Thor synthesized oracles"));
        assert!(with_oracles.contains("toBe(200)"));

        let yaml = TestGenerator::quarantine_yaml(&[("flaky".into(), "skip".into(), "rate high".into())]);
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
