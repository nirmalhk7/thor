use super::{SecurityEngine, SsrfLabReport, SsrfTarget};

impl SecurityEngine {
    /// Consent-aware SSRF lab: labels blocked vs lab-allowed targets using the
    /// same private/local + abuse-block helpers as fuzz gating.
    /// Returns `Err` when any non-local public target lacks `lab_consent`.
    pub fn ssrf_lab_harness(
        &self,
        targets: &[SsrfTarget],
        lab_consent: bool,
    ) -> Result<SsrfLabReport, String> {
        check_ssrf_consent(targets, lab_consent)?;
        let mut report = SsrfLabReport {
            blocked: Vec::new(),
            allowed_lab: Vec::new(),
            issues: Vec::new(),
        };
        for target in targets {
            record_ssrf_target(target, lab_consent, &mut report);
        }
        Ok(report)
    }
}
fn check_ssrf_consent(targets: &[SsrfTarget], lab_consent: bool) -> Result<(), String> {
    for t in targets {
        if crate::net::consent::is_blocked_abuse_target(&t.url) {
            continue;
        }
        if !crate::net::consent::is_private_or_local_url(&t.url) && !lab_consent {
            return Err(format!(
                "SSRF lab refused: public target without lab_consent: {}",
                t.url
            ));
        }
    }
    Ok(())
}

fn record_ssrf_target(t: &SsrfTarget, lab_consent: bool, report: &mut SsrfLabReport) {
    if crate::net::consent::is_blocked_abuse_target(&t.url) {
        report.blocked.push(t.url.clone());
        report
            .issues
            .push(format!("blocked abuse/metadata target {}", t.url));
        return;
    }
    if crate::net::consent::is_private_or_local_url(&t.url) {
        report.allowed_lab.push(t.url.clone());
    } else if lab_consent {
        report.allowed_lab.push(t.url.clone());
        report
            .issues
            .push(format!("lab-consented public target {}", t.url));
    } else {
        report.blocked.push(t.url.clone());
        report.issues.push(format!(
            "non-local target blocked without explicit lab consent: {}",
            t.url
        ));
    }
    record_sensitive_ip(t, &mut report.issues);
}

fn record_sensitive_ip(t: &SsrfTarget, issues: &mut Vec<String>) {
    if let Some(ip) = &t.resolved_ip {
        if ip.starts_with("169.254.") || ip == "0.0.0.0" || ip.starts_with("127.") {
            issues.push(format!("{} resolved to sensitive IP {ip}", t.url));
        }
    }
}
