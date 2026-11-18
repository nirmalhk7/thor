use super::{AiEngine, InjectionFinding};

impl AiEngine {
    // 10. Injection Scanner -------------------------------------------------
    pub fn scan_injection(&self, input: &str) -> Vec<InjectionFinding> {
        let lower = input.to_lowercase();
        let patterns: &[(&str, &str)] = &[
            ("ignore previous instructions", "high"),
            ("ignore all previous", "high"),
            ("disregard the above", "high"),
            ("system prompt", "medium"),
            ("you are now", "medium"),
            ("developer mode", "high"),
            ("do anything now", "high"),
            ("dan mode", "high"),
            ("reveal your prompt", "high"),
            ("act as", "low"),
            ("jailbreak", "high"),
            ("base64", "low"),
        ];
        let mut findings = Vec::new();
        for (pat, sev) in patterns {
            if let Some(pos) = lower.find(pat) {
                let end = (pos + pat.len() + 20).min(input.len());
                let start = pos.saturating_sub(10);
                findings.push(InjectionFinding {
                    pattern: pat.to_string(),
                    severity: sev.to_string(),
                    excerpt: safe_slice(input, start, end),
                    category: "injection".to_string(),
                });
            }
        }
        findings.extend(detect_pii(input));
        findings
    }
}

/// Detect leaked PII / secrets (emails, API keys, JWTs) in free text.
fn detect_pii(input: &str) -> Vec<InjectionFinding> {
    let mut findings = Vec::new();
    for token in input.split(|c: char| {
        c.is_whitespace() || matches!(c, ',' | ';' | '"' | '\'' | '(' | ')' | '<' | '>')
    }) {
        let t = token.trim();
        if t.len() < 6 {
            continue;
        }
        let (pattern, severity) = if is_email(t) {
            ("pii:email", "medium")
        } else if is_jwt(t) {
            ("pii:jwt", "high")
        } else if is_api_key(t) {
            ("pii:api-key", "high")
        } else {
            continue;
        };
        findings.push(InjectionFinding {
            pattern: pattern.to_string(),
            severity: severity.to_string(),
            excerpt: redact(t),
            category: "pii".to_string(),
        });
    }
    findings
}

fn is_email(t: &str) -> bool {
    let at = match t.find('@') {
        Some(i) if i > 0 => i,
        _ => return false,
    };
    let domain = &t[at + 1..];
    domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn is_jwt(t: &str) -> bool {
    let parts: Vec<&str> = t.split('.').collect();
    let valid_parts = parts.len() == 3 && parts.iter().all(|p| valid_jwt_segment(p));
    valid_parts && jwt_prefix(t)
}

fn valid_jwt_segment(segment: &str) -> bool {
    segment.len() >= 4 && token_characters(segment)
}

fn jwt_prefix(token: &str) -> bool {
    token.starts_with("eyJ") || token.starts_with("ey")
}

fn token_characters(token: &str) -> bool {
    token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_api_key(t: &str) -> bool {
    const PREFIXES: [&str; 6] = ["sk-", "sk_live_", "sk_test_", "pk_live_", "ghp_", "AKIA"];
    if PREFIXES.iter().any(|p| t.starts_with(p)) && t.len() >= 12 {
        return true;
    }
    long_key_shape(t) && key_character_classes(t)
}

fn long_key_shape(token: &str) -> bool {
    token.len() >= 32 && token_characters(token)
}

fn key_character_classes(token: &str) -> bool {
    token.chars().any(|c| c.is_ascii_digit()) && token.chars().any(|c| c.is_ascii_alphabetic())
}

fn redact(t: &str) -> String {
    if t.len() <= 6 {
        return "***".to_string();
    }
    let head: String = t.chars().take(3).collect();
    let tail: String = t
        .chars()
        .rev()
        .take(2)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}***{tail}")
}

fn safe_slice(s: &str, start: usize, end: usize) -> String {
    let start = start.min(s.len());
    let end = end.min(s.len()).max(start);
    let mut a = start;
    while a > 0 && !s.is_char_boundary(a) {
        a -= 1;
    }
    let mut b = end;
    while b < s.len() && !s.is_char_boundary(b) {
        b += 1;
    }
    s[a..b].to_string()
}
