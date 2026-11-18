use super::{OidcConfig, OidcMisconfigReport, SecurityEngine};

impl SecurityEngine {
    pub fn oauth_oidc_misconfig(&self, cfg: &OidcConfig) -> OidcMisconfigReport {
        let mut issues: Vec<String> = Vec::new();
        if cfg.authorization_endpoint.starts_with("http://") {
            issues.push("authorization_endpoint not HTTPS".into());
        }
        if cfg.token_endpoint.starts_with("http://") {
            issues.push("token_endpoint not HTTPS".into());
        }
        if cfg.jwks_uri.is_none() {
            issues.push("missing jwks_uri".into());
        }
        if cfg.response_types_supported.iter().any(|r| r == "token") {
            issues.push("implicit flow (response_type=token) enabled".into());
        }
        if cfg.grant_types_supported.iter().any(|g| g == "password") {
            issues.push("resource-owner password credentials grant enabled".into());
        }
        let severity = if issues
            .iter()
            .any(|i| i.contains("not HTTPS") || i.contains("password"))
        {
            "high"
        } else if issues.is_empty() {
            "none"
        } else {
            "medium"
        };
        OidcMisconfigReport {
            issues,
            severity: severity.to_string(),
        }
    }
}
