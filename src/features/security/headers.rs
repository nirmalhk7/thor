use super::{BTreeMap, HeaderFinding, HeaderReport, SecurityEngine};

impl SecurityEngine {
    // 3. Header Analyzer ----------------------------------------------------
    pub fn analyze_headers(&self, headers: &BTreeMap<String, String>) -> HeaderReport {
        let lower: BTreeMap<String, String> = headers
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect();
        let mut findings = Vec::new();
        let mut score = required_header_score(&lower, &mut findings);
        score -= leak_penalty(&lower, &mut findings);
        score -= csp_penalty(lower.get("content-security-policy"), &mut findings);
        score -= cors_penalty(&lower, &mut findings);
        let score = score.max(0) as u32;
        let grade = match score {
            90..=100 => "A",
            75..=89 => "B",
            60..=74 => "C",
            40..=59 => "D",
            _ => "F",
        };
        HeaderReport {
            findings,
            grade: grade.to_string(),
            score,
        }
    }
}
fn required_header_score(
    lower: &BTreeMap<String, String>,
    findings: &mut Vec<HeaderFinding>,
) -> i64 {
    // (header, severity_weight, recommendation)
    let required = [
        (
            "content-security-policy",
            30u32,
            "Add a restrictive CSP to mitigate XSS",
        ),
        (
            "strict-transport-security",
            20,
            "Add HSTS with a long max-age",
        ),
        (
            "x-content-type-options",
            10,
            "Set X-Content-Type-Options: nosniff",
        ),
        (
            "x-frame-options",
            15,
            "Set X-Frame-Options or CSP frame-ancestors",
        ),
        ("referrer-policy", 10, "Set a Referrer-Policy"),
        (
            "permissions-policy",
            5,
            "Restrict powerful features via Permissions-Policy",
        ),
    ];
    let mut score = 100i64;
    for (h, weight, rec) in required {
        let present = lower.contains_key(h);
        if !present {
            score -= weight as i64;
        }
        findings.push(HeaderFinding {
            header: h.to_string(),
            present,
            severity: if weight >= 20 {
                "high".into()
            } else if weight >= 10 {
                "medium".into()
            } else {
                "low".into()
            },
            recommendation: if present {
                "OK".into()
            } else {
                rec.to_string()
            },
        });
    }
    score
}

fn leak_penalty(lower: &BTreeMap<String, String>, findings: &mut Vec<HeaderFinding>) -> i64 {
    let mut score = 0i64;
    for leak in ["server", "x-powered-by"] {
        if lower.contains_key(leak) {
            score -= 5;
            findings.push(HeaderFinding {
                header: leak.to_string(),
                present: true,
                severity: "low".into(),
                recommendation: format!("Remove `{leak}` to avoid version disclosure"),
            });
        }
    }
    -score
}

fn csp_penalty(csp: Option<&String>, findings: &mut Vec<HeaderFinding>) -> i64 {
    let Some(csp) = csp else {
        return 0;
    };
    let mut score = 0i64;
    let c = csp.to_lowercase();
    if c.contains("unsafe-inline") {
        score -= 10;
        findings.push(HeaderFinding {
            header: "content-security-policy".into(),
            present: true,
            severity: "medium".into(),
            recommendation: "CSP allows 'unsafe-inline' — weakens XSS mitigation".into(),
        });
    }
    if c.contains("unsafe-eval") {
        score -= 10;
        findings.push(HeaderFinding {
            header: "content-security-policy".into(),
            present: true,
            severity: "medium".into(),
            recommendation: "CSP allows 'unsafe-eval' — enables string-to-code".into(),
        });
    }
    let wildcard_source = c.split(';').any(|d| {
        let d = d.trim();
        d.ends_with(" *") || d.ends_with(":*") || d.split_whitespace().any(|t| t == "*")
    });
    if wildcard_source {
        score -= 5;
        findings.push(HeaderFinding {
            header: "content-security-policy".into(),
            present: true,
            severity: "low".into(),
            recommendation: "CSP uses a wildcard (*) source — tighten allowed origins".into(),
        });
    }
    -score
}

fn cors_penalty(lower: &BTreeMap<String, String>, findings: &mut Vec<HeaderFinding>) -> i64 {
    let Some(acao) = lower.get("access-control-allow-origin") else {
        return 0;
    };
    let mut score = 0i64;
    let creds = lower
        .get("access-control-allow-credentials")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if acao.trim() == "*" && creds {
        score -= 20;
        findings.push(HeaderFinding {
            header: "access-control-allow-origin".into(),
            present: true,
            severity: "high".into(),
            recommendation: "CORS: wildcard ACAO with Allow-Credentials — reflects any origin"
                .into(),
        });
    } else if acao.trim() == "*" {
        findings.push(HeaderFinding {
            header: "access-control-allow-origin".into(),
            present: true,
            severity: "low".into(),
            recommendation: "CORS: wildcard ACAO exposes the endpoint to any origin".into(),
        });
    }
    -score
}
