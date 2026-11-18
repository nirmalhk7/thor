use super::{
    CookieForensicsReport, CookieJarEntry, SecurityEngine, SessionFixationProbe,
    SessionFixationReport,
};

impl SecurityEngine {
    pub fn cookie_session_forensics(&self, cookies: &[CookieJarEntry]) -> CookieForensicsReport {
        let mut issues = Vec::new();
        let mut session_cookies = 0usize;
        for cookie in cookies {
            if is_session_cookie(cookie) {
                session_cookies += 1;
                inspect_session_cookie(cookie, &mut issues);
            }
            if cookie.value.len() > 4096 {
                issues.push(format!("{} value exceeds 4KB", cookie.name));
            }
        }
        CookieForensicsReport {
            session_cookies,
            issues,
        }
    }

    /// Consent-aware: only flags fixation when login succeeds; always requires
    /// `target_url` and gates via `require_lab_consent` before analysis.
    pub fn session_fixation_lab(
        &self,
        probe: &SessionFixationProbe,
    ) -> Result<SessionFixationReport, String> {
        if probe.target_url.trim().is_empty() {
            return Err("session-fixation-lab requires target_url".into());
        }
        crate::net::consent::require_lab_consent(&probe.target_url, probe.lab_consent)?;
        if probe.login_status < 200 || probe.login_status >= 400 {
            return Ok(SessionFixationReport {
                vulnerable: false,
                detail: format!("login did not succeed (status {})", probe.login_status),
            });
        }
        let vulnerable =
            !probe.pre_auth_session.is_empty() && probe.pre_auth_session == probe.post_auth_session;
        Ok(SessionFixationReport {
            vulnerable,
            detail: if vulnerable {
                "session id unchanged across authentication".into()
            } else {
                "session rotated on login".into()
            },
        })
    }
}
fn is_session_cookie(cookie: &CookieJarEntry) -> bool {
    cookie.name.to_lowercase().contains("session")
        || cookie.name.to_lowercase().contains("sid")
        || cookie.max_age.is_none()
}

fn inspect_session_cookie(c: &CookieJarEntry, issues: &mut Vec<String>) {
    if !c.secure {
        issues.push(format!("{} missing Secure", c.name));
    }
    if !c.http_only {
        issues.push(format!("{} missing HttpOnly", c.name));
    }
    match c.same_site.as_deref().map(|s| s.to_lowercase()) {
        Some(s) if s == "strict" || s == "lax" || s == "none" => {
            if s == "none" && !c.secure {
                issues.push(format!("{} SameSite=None without Secure", c.name));
            }
        }
        _ => issues.push(format!("{} missing/invalid SameSite", c.name)),
    }
}
