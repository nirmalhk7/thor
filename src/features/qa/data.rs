use super::{AuthzTestCase, DeterminismControls, DeterminismReport, HashMap, QaEngine};

impl QaEngine {
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
}
