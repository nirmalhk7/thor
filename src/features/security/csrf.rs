use super::{CsrfContext, CsrfReport, SecurityEngine};

impl SecurityEngine {
    // 9. CSRF Analyzer ------------------------------------------------------
    pub fn analyze_csrf(&self, ctx: &CsrfContext) -> CsrfReport {
        let method = ctx.method.to_uppercase();
        let state_changing = matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
        let mut issues = Vec::new();
        if !state_changing {
            return CsrfReport {
                vulnerable: false,
                issues,
            };
        }
        if !ctx.uses_cookie_auth {
            // Non-cookie auth (bearer) is not automatically sent by the browser.
            return CsrfReport {
                vulnerable: false,
                issues,
            };
        }
        let same_site_ok = ctx
            .cookie_same_site
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("lax") || s.eq_ignore_ascii_case("strict"))
            .unwrap_or(false);
        if !ctx.has_csrf_token {
            issues.push(
                "no anti-CSRF token on a state-changing, cookie-authenticated request".into(),
            );
        }
        if !same_site_ok {
            issues.push("session cookie SameSite is not Lax/Strict".into());
        }
        let is_simple_ct = ctx
            .content_type
            .as_deref()
            .map(|c| {
                c.contains("application/x-www-form-urlencoded")
                    || c.contains("multipart/form-data")
                    || c.contains("text/plain")
            })
            .unwrap_or(true);
        if is_simple_ct && !ctx.has_csrf_token {
            issues.push("'simple' content-type allows cross-site form submission".into());
        }
        CsrfReport {
            vulnerable: !issues.is_empty(),
            issues,
        }
    }
}
