use super::{A11ySpec, A11yViolation, QaEngine, TestGenerator};

impl QaEngine {
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
}
