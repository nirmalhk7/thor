use super::{
    CiBundleManifest, ExportFormat, HashMap, IdentityFixture, OracleObservation, QaEngine,
    RecordedSession, TestGenerator,
};

impl QaEngine {
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
            playwright_ts = TestGenerator::promote_to_test(session, ExportFormat::PlaywrightTs);
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
        let a11y_scaffold = TestGenerator::a11y_spec_scaffold(&session.name, &[]);
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
}
