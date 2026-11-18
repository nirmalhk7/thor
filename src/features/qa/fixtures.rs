use super::{IdentityFixture, QaEngine};

impl QaEngine {
    // 4. Identity Fixtures --------------------------------------------------
    pub fn add_fixture(&self, fixture: IdentityFixture) {
        self.fixtures.lock().insert(fixture.name.clone(), fixture);
    }

    pub fn fixture(&self, name: &str) -> Option<IdentityFixture> {
        self.fixtures.lock().get(name).cloned()
    }

    /// Emit a Playwright `storageState`-style setup snippet for a fixture.
    pub fn fixture_setup_script(&self, name: &str) -> Option<String> {
        let f = self.fixture(name)?;
        let cookies: Vec<serde_json::Value> = f
            .cookies
            .iter()
            .map(|(k, v)| serde_json::json!({ "name": k, "value": v }))
            .collect();
        let origins: Vec<serde_json::Value> = if f.tokens.is_empty() {
            vec![]
        } else {
            vec![serde_json::json!({
                "origin": "",
                "localStorage": f.tokens.iter().map(|(k, v)| serde_json::json!({ "name": k, "value": v })).collect::<Vec<_>>()
            })]
        };
        let state = serde_json::json!({ "cookies": cookies, "origins": origins });
        Some(serde_json::to_string_pretty(&state).unwrap_or_default())
    }
}
