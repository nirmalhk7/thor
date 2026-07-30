//! Consent & scope helpers for high-risk lab tools (Locust, tamper).
//!
//! Rust is the security boundary — chrome/content JS cannot bypass these checks.

use crate::net::locust::LocustConfig;

/// Hard caps (SEC-062). Public targets use the stricter defaults.
pub const LOCUST_MAX_USERS: u32 = 100;
pub const LOCUST_DEFAULT_USERS_CAP: u32 = 10;
pub const LOCUST_MAX_RAMP_SECS: u32 = 60;
pub const LOCUST_MAX_RUNTIME_SECS: u64 = 300;

fn host_from_url(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("HTTPS://"))
        .or_else(|| url.strip_prefix("HTTP://"))
        .unwrap_or(url);
    let hostport = rest.split('/').next()?.split('?').next()?;
    let host = if hostport.starts_with('[') {
        hostport
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or(hostport)
    } else {
        hostport.split(':').next().unwrap_or(hostport)
    };
    Some(host.to_lowercase())
}

/// True for loopback, RFC1918, `.localhost`, `.test`, `dev.local`.
pub fn is_private_or_local_url(url: &str) -> bool {
    let Some(host) = host_from_url(url) else {
        return false;
    };
    if host == "localhost" || host == "127.0.0.1" || host == "::1" {
        return true;
    }
    if host.ends_with(".localhost") || host.ends_with(".test") || host == "dev.local" {
        return true;
    }
    if let Ok(std::net::IpAddr::V4(v4)) = host.parse::<std::net::IpAddr>() {
        let o = v4.octets();
        if o[0] == 10 || o[0] == 127 {
            return true;
        }
        if o[0] == 172 && (16..=31).contains(&o[1]) {
            return true;
        }
        if o[0] == 192 && o[1] == 168 {
            return true;
        }
    }
    false
}

/// Block obvious cloud-metadata / link-local abuse targets (SEC-066).
pub fn is_blocked_abuse_target(url: &str) -> bool {
    let lower = url.to_lowercase();
    let host = host_from_url(url).unwrap_or_default();
    lower.contains("169.254.169.254")
        || lower.contains("metadata.google")
        || lower.contains("metadata.goog")
        || lower.contains("metadata.azure.com")
        || lower.contains("instance-data")
        || lower.contains("100.100.100.200")
        || lower.contains("168.63.129.16")
        || lower.contains("[fd00:ec2::254]")
        || lower.contains("fd00:ec2::254")
        || host == "metadata"
        || host.ends_with(".internal") && lower.contains("metadata")
        || host == "kubernetes.default"
        || host == "kubernetes.default.svc"
}

/// Require explicit lab consent for non-local public targets.
pub fn require_lab_consent(url: &str, lab_consent: bool) -> Result<(), String> {
    if is_blocked_abuse_target(url) {
        return Err(format!("blocked abuse/metadata target: {url}"));
    }
    if !is_private_or_local_url(url) && !lab_consent {
        return Err(format!(
            "public target requires lab_consent (authorized testing only): {url}"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct LocustStartGate {
    pub target_url: String,
    pub users: u32,
    pub ramp_up_secs: u32,
    pub runtime_secs: u64,
    /// Required when target is not private/local.
    pub public_ack: bool,
}

/// Validate Locust start parameters. Does not start the swarm.
pub fn validate_locust_start(gate: LocustStartGate) -> Result<LocustConfig, String> {
    let url = gate.target_url.trim();
    if url.is_empty() {
        return Err("Locust target URL is empty".into());
    }
    if is_blocked_abuse_target(url) {
        return Err("Locust target blocked (metadata / link-local abuse)".into());
    }

    let private = is_private_or_local_url(url);
    if !private && !gate.public_ack {
        return Err(
            "Public Locust target requires explicit public_ack (authorized testing only)".into(),
        );
    }

    let user_cap = if private {
        LOCUST_MAX_USERS
    } else {
        LOCUST_DEFAULT_USERS_CAP
    };
    if gate.users > user_cap {
        return Err(format!(
            "Locust users {} exceeds cap {}",
            gate.users, user_cap
        ));
    }
    if gate.ramp_up_secs > LOCUST_MAX_RAMP_SECS {
        return Err(format!(
            "Locust ramp {}s exceeds cap {}s",
            gate.ramp_up_secs, LOCUST_MAX_RAMP_SECS
        ));
    }
    if gate.runtime_secs > LOCUST_MAX_RUNTIME_SECS {
        return Err(format!(
            "Locust runtime {}s exceeds cap {}s",
            gate.runtime_secs, LOCUST_MAX_RUNTIME_SECS
        ));
    }

    let users = gate.users.max(1);
    let ramp = gate.ramp_up_secs;
    let runtime = gate.runtime_secs.max(1);
    Ok(LocustConfig::new(url, users, ramp, runtime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_hosts_detected() {
        assert!(is_private_or_local_url("http://127.0.0.1:8080/"));
        assert!(is_private_or_local_url("http://localhost/"));
        assert!(is_private_or_local_url("https://app.localhost/"));
        assert!(is_private_or_local_url("http://192.168.1.10/api"));
        assert!(is_private_or_local_url("http://10.0.0.2/"));
        assert!(!is_private_or_local_url("https://example.com/"));
    }

    #[test]
    fn public_requires_ack() {
        assert!(validate_locust_start(LocustStartGate {
            target_url: "https://example.com/".into(),
            users: 5,
            ramp_up_secs: 1,
            runtime_secs: 10,
            public_ack: false,
        })
        .is_err());

        assert!(validate_locust_start(LocustStartGate {
            target_url: "https://example.com/".into(),
            users: 5,
            ramp_up_secs: 1,
            runtime_secs: 10,
            public_ack: true,
        })
        .is_ok());
    }

    #[test]
    fn blocked_abuse_targets_expanded() {
        assert!(is_blocked_abuse_target("http://169.254.169.254/latest/meta-data/"));
        assert!(is_blocked_abuse_target("http://metadata.google.internal/"));
        assert!(is_blocked_abuse_target("http://100.100.100.200/latest/meta-data/"));
        assert!(is_blocked_abuse_target("http://168.63.129.16/metadata/instance"));
        assert!(is_blocked_abuse_target("http://[fd00:ec2::254]/latest/meta-data/"));
        assert!(!is_blocked_abuse_target("https://example.com/"));
    }

    #[test]
    fn lab_consent_gate() {
        assert!(require_lab_consent("https://example.com/", false).is_err());
        assert!(require_lab_consent("https://example.com/", true).is_ok());
        assert!(require_lab_consent("http://127.0.0.1:9/", false).is_ok());
        assert!(require_lab_consent("http://169.254.169.254/", true).is_err());
    }

    #[test]
    fn caps_enforced() {
        assert!(validate_locust_start(LocustStartGate {
            target_url: "http://127.0.0.1/".into(),
            users: 10_000,
            ramp_up_secs: 0,
            runtime_secs: 5,
            public_ack: false,
        })
        .is_err());
    }
}
