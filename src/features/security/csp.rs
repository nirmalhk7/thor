use super::{BTreeMap, CspForensicsReport, SecurityEngine};

impl SecurityEngine {
    pub fn csp_policy_forensics(&self, policy: &str) -> CspForensicsReport {
        let mut directives: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for part in policy.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let mut toks = part.split_whitespace();
            if let Some(name) = toks.next() {
                directives.insert(name.to_lowercase(), toks.map(|s| s.to_string()).collect());
            }
        }
        let mut issues = Vec::new();
        let mut score = 100u32;
        let unsafe_tokens = ["'unsafe-inline'", "'unsafe-eval'", "data:", "*"];
        for (dir, vals) in &directives {
            for v in vals {
                if unsafe_tokens.iter().any(|t| v.contains(t)) {
                    issues.push(format!("{dir} contains {v}"));
                    score = score.saturating_sub(15);
                }
            }
        }
        if !directives.contains_key("default-src") && !directives.contains_key("script-src") {
            issues.push("missing default-src and script-src".into());
            score = score.saturating_sub(25);
        }
        if !directives.contains_key("object-src") {
            issues.push("consider object-src 'none'".into());
            score = score.saturating_sub(5);
        }
        CspForensicsReport {
            directives,
            issues,
            score,
        }
    }
}
