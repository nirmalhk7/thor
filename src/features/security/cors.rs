use super::{CorsDeepReport, CorsProbeResult, SecurityEngine};

impl SecurityEngine {
    pub fn cors_deep_probe(&self, probes: &[CorsProbeResult]) -> CorsDeepReport {
        let mut report = CorsDeepReport {
            findings: Vec::new(),
            reflects_origin: false,
            credentials_with_star: false,
            credentials_with_reflected_origin: false,
        };
        for probe in probes {
            record_cors_probe(probe, &mut report);
        }
        report
    }
}
fn record_cors_probe(probe: &CorsProbeResult, report: &mut CorsDeepReport) {
    let Some(acao) = &probe.acao else {
        return;
    };
    if acao == "*" {
        report
            .findings
            .push(format!("ACAO * for origin {}", probe.origin));
        if probe.acac.as_deref() == Some("true") {
            report.credentials_with_star = true;
            report
                .findings
                .push("ACAO * with credentials (invalid/dangerous)".into());
        }
    } else if acao == &probe.origin {
        record_reflected_origin(probe, report);
    }
}

fn record_reflected_origin(probe: &CorsProbeResult, report: &mut CorsDeepReport) {
    report.reflects_origin = true;
    if probe.origin == "null" {
        report.findings.push("reflects Origin: null".into());
    }
    if probe.acac.as_deref() == Some("true") {
        report.credentials_with_reflected_origin = true;
        report.findings.push(format!(
            "reflected origin {} with Access-Control-Allow-Credentials: true",
            probe.origin
        ));
    }
}
