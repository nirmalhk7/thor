use super::{FuzzCase, FuzzCategory, SecurityEngine};

impl SecurityEngine {
    // 8. Fuzz Repeater ------------------------------------------------------
    pub fn fuzz_payloads(category: FuzzCategory) -> Vec<&'static str> {
        match category {
            FuzzCategory::Sqli => vec![
                "' OR '1'='1",
                "'; DROP TABLE users;--",
                "\" OR 1=1--",
                "1 UNION SELECT NULL--",
            ],
            FuzzCategory::Xss => vec![
                "<script>alert(1)</script>",
                "\"><img src=x onerror=alert(1)>",
                "javascript:alert(1)",
            ],
            FuzzCategory::PathTraversal => vec![
                "../../../../etc/passwd",
                "..%2f..%2f..%2fetc%2fpasswd",
                "....//....//etc/passwd",
            ],
            FuzzCategory::CommandInjection => vec!["; id", "| whoami", "$(id)", "`id`"],
            FuzzCategory::Ssti => vec!["{{7*7}}", "${7*7}", "<%= 7*7 %>", "#{7*7}"],
        }
    }

    /// Build fuzz cases by substituting `FUZZ` in a template with each payload.
    pub fn build_fuzz_cases(&self, template: &str, category: FuzzCategory) -> Vec<FuzzCase> {
        let cat = format!("{category:?}");
        Self::fuzz_payloads(category)
            .into_iter()
            .map(|payload| FuzzCase {
                category: cat.clone(),
                payload: payload.to_string(),
                mutated: template.replace("FUZZ", payload),
            })
            .collect()
    }

    /// Consent-gated fuzzing: refuse to generate active attack payloads against a
    /// target that is not private/local (authorized local testing only, per the
    /// `net/consent` scope boundary). `target_url` is the endpoint under test.
    pub fn build_fuzz_cases_gated(
        &self,
        template: &str,
        category: FuzzCategory,
        target_url: &str,
    ) -> Result<Vec<FuzzCase>, String> {
        if !crate::net::consent::is_private_or_local_url(target_url) {
            return Err(format!(
                "Fuzzing blocked: {target_url} is not private/local (authorized local targets only)"
            ));
        }
        if crate::net::consent::is_blocked_abuse_target(target_url) {
            return Err("Fuzzing blocked: metadata / link-local abuse target".into());
        }
        Ok(self.build_fuzz_cases(template, category))
    }
}
