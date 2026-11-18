use super::{regex_escape, TestGenerator};

impl TestGenerator {
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
                let name = rest.trim().trim_matches('"').trim_matches('\'').to_string();
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
}
