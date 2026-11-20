use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewInput {
    pub headers: Vec<HeaderEvidence>,
    #[serde(default)]
    pub evidence_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderEvidence {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct ReviewReport {
    pub version: u8,
    pub source: String,
    pub evidence_name: String,
    pub redacted: bool,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Serialize)]
pub struct Finding {
    pub rule_id: &'static str,
    pub title: &'static str,
    pub severity: &'static str,
    pub description: &'static str,
    pub evidence: String,
}

const SENSITIVE_HEADERS: &[&str] = &[
    "set-cookie",
    "cookie",
    "authorization",
    "proxy-authorization",
    "x-api-key",
];

/// Analyze only headers explicitly supplied by the user. This function has no I/O.
pub fn review(input: ReviewInput) -> Result<ReviewReport, String> {
    if input.headers.len() > 512
        || input
            .headers
            .iter()
            .any(|h| h.name.len() > 256 || h.value.len() > 16_384)
    {
        return Err("Evidence exceeds the 512-header / 16 KiB per-value limit".into());
    }
    let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for header in input.headers {
        let name = header.name.trim().to_ascii_lowercase();
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        {
            return Err("Evidence contains an invalid header name".into());
        }
        if header
            .value
            .bytes()
            .any(|byte| matches!(byte, b'\r' | b'\n' | 0))
        {
            return Err("Evidence contains an invalid header value".into());
        }
        headers
            .entry(name)
            .or_default()
            .push(header.value.trim().to_owned());
    }
    let has = |name: &str| headers.contains_key(name);
    let mut findings = Vec::new();
    for (name, title, rule, description) in [
        (
            "content-security-policy",
            "Content Security Policy is absent",
            "HDR-001",
            "Consider a restrictive Content-Security-Policy appropriate for the application.",
        ),
        (
            "strict-transport-security",
            "HTTP Strict Transport Security is absent",
            "HDR-002",
            "For HTTPS sites, consider a suitable Strict-Transport-Security policy.",
        ),
        (
            "x-content-type-options",
            "MIME sniffing protection is absent",
            "HDR-003",
            "Set X-Content-Type-Options: nosniff where appropriate.",
        ),
        (
            "referrer-policy",
            "Referrer Policy is absent",
            "HDR-004",
            "Set a Referrer-Policy that limits sensitive URL leakage.",
        ),
        (
            "permissions-policy",
            "Permissions Policy is absent",
            "HDR-005",
            "Consider restricting browser features that the application does not use.",
        ),
    ] {
        if !has(name) {
            findings.push(Finding {
                rule_id: rule,
                title,
                severity: "low",
                description,
                evidence: format!("Missing response header: {name}"),
            });
        }
    }
    for value in headers.get("set-cookie").into_iter().flatten() {
        let lower = value.to_ascii_lowercase();
        let cookie_name = value
            .split(';')
            .next()
            .and_then(|part| part.split('=').next())
            .unwrap_or("cookie")
            .trim();
        let label = if cookie_name.is_empty() {
            "cookie"
        } else {
            cookie_name
        };
        if !lower.split(';').any(|part| part.trim() == "secure") {
            findings.push(cookie_finding(
                "CK-001",
                "Cookie lacks Secure",
                "Set the Secure attribute when a cookie should only be sent over HTTPS.",
                label,
            ));
        }
        if !lower.split(';').any(|part| part.trim() == "httponly") {
            findings.push(cookie_finding(
                "CK-002",
                "Cookie lacks HttpOnly",
                "Set HttpOnly when client-side scripts do not need to read the cookie.",
                label,
            ));
        }
        if !lower
            .split(';')
            .any(|part| part.trim_start().starts_with("samesite="))
        {
            findings.push(cookie_finding(
                "CK-003",
                "Cookie lacks SameSite",
                "Set an appropriate SameSite attribute to limit cross-site cookie sending.",
                label,
            ));
        }
    }
    // Secret-bearing header values are never copied to results, regardless of display preference.
    let evidence = headers
        .keys()
        .map(|name| {
            if SENSITIVE_HEADERS.contains(&name.as_str()) {
                format!("{name}: [REDACTED]")
            } else {
                format!("{name}: [value omitted]")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !evidence.is_empty() {
        findings.push(Finding { rule_id: "EVIDENCE-001", title: "Supplied header evidence", severity: "info", description: "Header values are omitted from this report to protect credentials and personal data.", evidence });
    }
    Ok(ReviewReport {
        version: 1,
        source: "user-supplied response headers".into(),
        evidence_name: input
            .evidence_name
            .unwrap_or_else(|| "Imported response evidence".into())
            .chars()
            .take(120)
            .collect(),
        redacted: true,
        findings,
    })
}

fn cookie_finding(
    rule_id: &'static str,
    title: &'static str,
    description: &'static str,
    name: &str,
) -> Finding {
    Finding {
        rule_id,
        title,
        severity: "medium",
        description,
        evidence: format!(
            "Cookie `{}` attributes were checked; its value is omitted.",
            name.chars().take(80).collect::<String>()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reviews_supplied_headers_without_echoing_credentials() {
        let report = review(ReviewInput {
            headers: vec![
                HeaderEvidence {
                    name: "Set-Cookie".into(),
                    value: "session=super-secret-cookie".into(),
                },
                HeaderEvidence {
                    name: "Authorization".into(),
                    value: "Bearer secret-token".into(),
                },
            ],
            evidence_name: None,
        })
        .unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("super-secret-cookie"));
        assert!(!json.contains("secret-token"));
        assert!(json.contains("CK-001"));
        assert!(json.contains("HDR-001"));
        assert!(report.redacted);
    }
    #[test]
    fn accepts_hardened_cookie_and_present_headers() {
        let headers = [
            "content-security-policy",
            "strict-transport-security",
            "x-content-type-options",
            "referrer-policy",
            "permissions-policy",
        ]
        .into_iter()
        .map(|name| HeaderEvidence {
            name: name.into(),
            value: "safe".into(),
        })
        .chain(std::iter::once(HeaderEvidence {
            name: "Set-Cookie".into(),
            value: "session=x; Secure; HttpOnly; SameSite=Lax".into(),
        }))
        .collect();
        let report = review(ReviewInput {
            headers,
            evidence_name: None,
        })
        .unwrap();
        assert!(!report
            .findings
            .iter()
            .any(|f| f.severity == "medium" || f.rule_id.starts_with("HDR-")));
    }
    #[test]
    fn rejects_malformed_and_oversized_supplied_evidence() {
        assert!(review(ReviewInput {
            headers: vec![HeaderEvidence {
                name: "Bad Name".into(),
                value: "x".into()
            }],
            evidence_name: None
        })
        .is_err());
        assert!(review(ReviewInput {
            headers: vec![HeaderEvidence {
                name: "X".into(),
                value: "x".repeat(16_385)
            }],
            evidence_name: None
        })
        .is_err());
    }
}
