//! Thor's process-level Chromium defaults.
//!
//! These settings are Thor policy, implemented without importing another
//! browser's code, patches, or resources.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::profiles::ProfileMode;

/// Privacy-oriented search engines offered by Thor helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacySearchEngine {
    DuckDuckGo,
    BraveSearch,
}

/// Chromium DNS-over-HTTPS mode. `Secure` never falls back to plaintext DNS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecureDnsMode {
    Off,
    Automatic,
    Secure,
}

impl SecureDnsMode {
    pub fn chromium_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Automatic => "automatic",
            Self::Secure => "secure",
        }
    }

    fn from_policy_value(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("off") => Self::Off,
            Some("automatic") => Self::Automatic,
            Some("secure") | None => Self::Secure,
            Some(_) => Self::Secure,
        }
    }
}

pub const CLOUDFLARE_DOH_TEMPLATE: &str = "https://cloudflare-dns.com/dns-query";
pub const QUAD9_DOH_TEMPLATE: &str = "https://dns.quad9.net/dns-query";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum DohProvider {
    Cloudflare,
    Quad9,
    Custom { https_template: String },
}

impl DohProvider {
    pub fn custom(template: impl Into<String>) -> Result<Self, String> {
        let https_template = template.into();
        validate_doh_template(&https_template)?;
        Ok(Self::Custom { https_template })
    }

    pub fn https_template(&self) -> &str {
        match self {
            Self::Cloudflare => CLOUDFLARE_DOH_TEMPLATE,
            Self::Quad9 => QUAD9_DOH_TEMPLATE,
            Self::Custom { https_template } => https_template,
        }
    }

    fn validated_or_default(&self) -> Self {
        if validate_doh_template(self.https_template()).is_ok() {
            self.clone()
        } else {
            Self::Cloudflare
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum DohPolicySource {
    ThorDefault,
    Environment { variables: Vec<String> },
    ManagedPolicy { policy_id: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DohHealthState {
    #[default]
    Unchecked,
    Healthy,
    Degraded,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DohHealthReadback {
    pub state: DohHealthState,
    pub checked_at_unix_ms: Option<u128>,
    pub detail: Option<String>,
    pub checked_template: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum DohProbeResult {
    Healthy { template: String },
    Degraded { template: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum DevHostSplitDnsPolicy {
    Disabled,
    DevHostOnly { domain_suffixes: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SplitDnsReadback {
    Disabled,
    DevHostOnly {
        domain_suffixes: Vec<String>,
    },
    SuppressedForTargetProfile {
        profile: ProfileMode,
        configured_domain_suffixes: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedDohPolicy {
    pub policy_id: String,
    pub mode: SecureDnsMode,
    pub provider: DohProvider,
    pub devhost_split_dns_suffixes: Vec<String>,
}

impl ManagedDohPolicy {
    pub fn new(
        policy_id: impl Into<String>,
        mode: SecureDnsMode,
        provider: DohProvider,
    ) -> Result<Self, String> {
        let policy_id = policy_id.into();
        if policy_id.trim().is_empty() {
            return Err("managed DoH policy ID is empty".into());
        }
        validate_doh_template(provider.https_template())?;
        Ok(Self {
            policy_id,
            mode,
            provider,
            devhost_split_dns_suffixes: Vec::new(),
        })
    }

    pub fn with_devhost_split_dns(mut self, domain_suffixes: Vec<String>) -> Result<Self, String> {
        self.devhost_split_dns_suffixes = normalize_split_dns_suffixes(&domain_suffixes)?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkPrivacyEnvironment {
    pub doh_mode: Option<String>,
    pub doh_provider: Option<String>,
    pub doh_template: Option<String>,
    pub webrtc_ip_policy: Option<String>,
}

impl NetworkPrivacyEnvironment {
    pub fn from_process() -> Self {
        Self {
            doh_mode: std::env::var("THOR_DOH_MODE").ok(),
            doh_provider: std::env::var("THOR_DOH_PROVIDER").ok(),
            doh_template: std::env::var("THOR_DOH_TEMPLATE").ok(),
            webrtc_ip_policy: std::env::var("THOR_WEBRTC_IP_POLICY").ok(),
        }
    }
}

/// Chromium's WebRTC address exposure policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebRtcIpPolicy {
    /// Use only the default public interface and do not expose private addresses.
    DefaultPublicInterfaceOnly,
    /// Require WebRTC UDP to traverse a configured proxy.
    DisableNonProxiedUdp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebRtcHealthState {
    #[default]
    Unchecked,
    Healthy,
    Degraded,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRtcHealthReadback {
    pub state: WebRtcHealthState,
    pub checked_at_unix_ms: Option<u128>,
    pub detail: Option<String>,
    pub observed_policy: Option<WebRtcIpPolicy>,
}

/// Result from a browser-backed WebRTC policy observation. Merely passing a
/// Chromium command-line switch is not an observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum WebRtcProbeResult {
    Observed {
        policy: WebRtcIpPolicy,
    },
    Degraded {
        observed_policy: Option<WebRtcIpPolicy>,
        reason: String,
    },
}

impl WebRtcIpPolicy {
    pub fn chromium_value(self) -> &'static str {
        match self {
            Self::DefaultPublicInterfaceOnly => "default_public_interface_only",
            Self::DisableNonProxiedUdp => "disable_non_proxied_udp",
        }
    }

    fn from_policy_value(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("disable_non_proxied_udp") => Self::DisableNonProxiedUdp,
            Some("default_public_interface_only") => Self::DefaultPublicInterfaceOnly,
            Some(_) | None => Self::DisableNonProxiedUdp,
        }
    }
}

/// Effective process-level network privacy policy passed to Chromium.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPrivacyPolicy {
    pub secure_dns_mode: SecureDnsMode,
    pub secure_dns_template: Option<String>,
    pub secure_dns_provider: Option<DohProvider>,
    pub secure_dns_source: DohPolicySource,
    #[serde(default, skip_deserializing)]
    secure_dns_health: DohHealthReadback,
    pub devhost_split_dns: DevHostSplitDnsPolicy,
    pub webrtc_ip_policy: WebRtcIpPolicy,
    #[serde(default, skip_deserializing)]
    webrtc_health: WebRtcHealthReadback,
}

impl NetworkPrivacyPolicy {
    pub const DEFAULT_DOH_TEMPLATE: &'static str = CLOUDFLARE_DOH_TEMPLATE;

    #[cfg(test)]
    fn from_values(
        doh_mode: Option<&str>,
        doh_template: Option<&str>,
        webrtc_ip_policy: Option<&str>,
    ) -> Self {
        let environment = NetworkPrivacyEnvironment {
            doh_mode: doh_mode.map(str::to_string),
            doh_provider: None,
            doh_template: doh_template.map(str::to_string),
            webrtc_ip_policy: webrtc_ip_policy.map(str::to_string),
        };
        Self::resolve(None, &environment)
    }

    fn from_environment() -> Self {
        Self::resolve(None, &NetworkPrivacyEnvironment::from_process())
    }

    /// Resolve policy with enterprise/managed DoH taking precedence over
    /// process environment. WebRTC remains independently environment-driven.
    pub fn resolve(
        managed_policy: Option<&ManagedDohPolicy>,
        environment: &NetworkPrivacyEnvironment,
    ) -> Self {
        let webrtc_ip_policy =
            WebRtcIpPolicy::from_policy_value(environment.webrtc_ip_policy.as_deref());
        let (secure_dns_mode, provider, source, devhost_split_dns) = match managed_policy {
            Some(managed) => resolve_managed_doh(managed),
            None => resolve_environment_doh(environment),
        };
        let secure_dns_template = if secure_dns_mode == SecureDnsMode::Off {
            None
        } else {
            provider
                .as_ref()
                .map(|provider| provider.https_template().to_string())
        };
        Self {
            secure_dns_mode,
            secure_dns_template,
            secure_dns_provider: provider,
            secure_dns_source: source,
            secure_dns_health: DohHealthReadback::default(),
            devhost_split_dns,
            webrtc_ip_policy,
            webrtc_health: WebRtcHealthReadback::default(),
        }
    }

    /// Record a result produced by a real probe owned by the caller. Command
    /// line configuration alone always remains `Unchecked`.
    pub fn record_verified_doh_probe(&mut self, result: DohProbeResult) -> Result<(), String> {
        let (checked_template, state, detail) = match result {
            DohProbeResult::Healthy { template } => (template, DohHealthState::Healthy, None),
            DohProbeResult::Degraded { template, reason } => (
                template,
                DohHealthState::Degraded,
                Some(if reason.trim().is_empty() {
                    "DoH probe reported degraded without detail".into()
                } else {
                    reason
                }),
            ),
        };
        if self.secure_dns_mode == SecureDnsMode::Off {
            return Err("cannot record DoH health while secure DNS is disabled".into());
        }
        let expected = self
            .secure_dns_template
            .as_deref()
            .ok_or_else(|| "cannot record DoH health while secure DNS is disabled".to_string())?;
        let checked_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("read DoH probe timestamp: {error}"))?
            .as_millis();
        if checked_template != expected || validate_doh_template(&checked_template).is_err() {
            self.secure_dns_health = DohHealthReadback {
                state: DohHealthState::Degraded,
                checked_at_unix_ms: Some(checked_at_unix_ms),
                detail: Some("probe result does not match the configured DoH template".into()),
                checked_template: Some(checked_template),
            };
            return Err("DoH probe template does not match configured policy".into());
        }
        self.secure_dns_health = DohHealthReadback {
            state,
            checked_at_unix_ms: Some(checked_at_unix_ms),
            detail,
            checked_template: Some(checked_template),
        };
        Ok(())
    }

    pub fn doh_health_readback(&self) -> DohHealthReadback {
        if self.secure_dns_health.state == DohHealthState::Healthy
            && (self.secure_dns_mode == SecureDnsMode::Off
                || self.secure_dns_health.checked_template.as_deref()
                    != self.secure_dns_template.as_deref())
        {
            return DohHealthReadback {
                state: DohHealthState::Degraded,
                checked_at_unix_ms: self.secure_dns_health.checked_at_unix_ms,
                detail: Some("recorded probe no longer matches configured DoH policy".into()),
                checked_template: self.secure_dns_health.checked_template.clone(),
            };
        }
        self.secure_dns_health.clone()
    }

    /// Record an observation produced by the running browser. Configuration
    /// and launcher arguments alone must leave this status `Unchecked`.
    pub fn record_verified_webrtc_probe(
        &mut self,
        result: WebRtcProbeResult,
    ) -> Result<(), String> {
        let checked_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("read WebRTC probe timestamp: {error}"))?
            .as_millis();
        match result {
            WebRtcProbeResult::Observed { policy } if policy == self.webrtc_ip_policy => {
                self.webrtc_health = WebRtcHealthReadback {
                    state: WebRtcHealthState::Healthy,
                    checked_at_unix_ms: Some(checked_at_unix_ms),
                    detail: None,
                    observed_policy: Some(policy),
                };
                Ok(())
            }
            WebRtcProbeResult::Observed { policy } => {
                self.webrtc_health = WebRtcHealthReadback {
                    state: WebRtcHealthState::Degraded,
                    checked_at_unix_ms: Some(checked_at_unix_ms),
                    detail: Some(
                        "observed WebRTC policy does not match the configured policy".into(),
                    ),
                    observed_policy: Some(policy),
                };
                Err("WebRTC probe does not match configured policy".into())
            }
            WebRtcProbeResult::Degraded {
                observed_policy,
                reason,
            } => {
                self.webrtc_health = WebRtcHealthReadback {
                    state: WebRtcHealthState::Degraded,
                    checked_at_unix_ms: Some(checked_at_unix_ms),
                    detail: Some(if reason.trim().is_empty() {
                        "WebRTC probe reported degraded without detail".into()
                    } else {
                        reason
                    }),
                    observed_policy,
                };
                Ok(())
            }
        }
    }

    pub fn webrtc_health_readback(&self) -> WebRtcHealthReadback {
        if self.webrtc_health.state == WebRtcHealthState::Healthy
            && self.webrtc_health.observed_policy != Some(self.webrtc_ip_policy)
        {
            return WebRtcHealthReadback {
                state: WebRtcHealthState::Degraded,
                checked_at_unix_ms: self.webrtc_health.checked_at_unix_ms,
                detail: Some("recorded probe no longer matches configured WebRTC policy".into()),
                observed_policy: self.webrtc_health.observed_policy,
            };
        }
        self.webrtc_health.clone()
    }

    pub fn split_dns_readback(&self, profile: ProfileMode) -> SplitDnsReadback {
        match (&self.devhost_split_dns, profile) {
            (DevHostSplitDnsPolicy::Disabled, _) => SplitDnsReadback::Disabled,
            (DevHostSplitDnsPolicy::DevHostOnly { domain_suffixes }, ProfileMode::DevHost) => {
                SplitDnsReadback::DevHostOnly {
                    domain_suffixes: domain_suffixes.clone(),
                }
            }
            (DevHostSplitDnsPolicy::DevHostOnly { domain_suffixes }, target_profile) => {
                SplitDnsReadback::SuppressedForTargetProfile {
                    profile: target_profile,
                    configured_domain_suffixes: domain_suffixes.clone(),
                }
            }
        }
    }

    pub fn chromium_args(&self) -> Vec<String> {
        let (secure_dns_mode, secure_dns_template) =
            match (self.secure_dns_mode, self.secure_dns_template.as_deref()) {
                (SecureDnsMode::Off, _) => (SecureDnsMode::Off, None),
                (mode, Some(template)) if validate_doh_template(template).is_ok() => {
                    (mode, Some(template))
                }
                _ => (SecureDnsMode::Secure, Some(CLOUDFLARE_DOH_TEMPLATE)),
            };
        let mut args = vec![
            format!(
                "--force-webrtc-ip-handling-policy={}",
                self.webrtc_ip_policy.chromium_value()
            ),
            format!("--dns-over-https-mode={}", secure_dns_mode.chromium_value()),
        ];
        if let Some(template) = secure_dns_template {
            args.push(format!("--dns-over-https-templates={template}"));
        }
        args
    }
}

fn resolve_managed_doh(
    managed: &ManagedDohPolicy,
) -> (
    SecureDnsMode,
    Option<DohProvider>,
    DohPolicySource,
    DevHostSplitDnsPolicy,
) {
    let policy_id = if managed.policy_id.trim().is_empty() {
        "invalid-managed-policy".to_string()
    } else {
        managed.policy_id.trim().to_string()
    };
    let source = DohPolicySource::ManagedPolicy { policy_id };
    let provider_valid = validate_doh_template(managed.provider.https_template()).is_ok();
    let split_dns = normalize_split_dns_suffixes(&managed.devhost_split_dns_suffixes);
    if managed.policy_id.trim().is_empty() || !provider_valid || split_dns.is_err() {
        return (
            SecureDnsMode::Secure,
            Some(DohProvider::Cloudflare),
            source,
            DevHostSplitDnsPolicy::Disabled,
        );
    }
    let provider = (managed.mode != SecureDnsMode::Off).then(|| managed.provider.clone());
    let devhost_split_dns = match split_dns.unwrap() {
        suffixes if suffixes.is_empty() => DevHostSplitDnsPolicy::Disabled,
        domain_suffixes => DevHostSplitDnsPolicy::DevHostOnly { domain_suffixes },
    };
    (managed.mode, provider, source, devhost_split_dns)
}

fn resolve_environment_doh(
    environment: &NetworkPrivacyEnvironment,
) -> (
    SecureDnsMode,
    Option<DohProvider>,
    DohPolicySource,
    DevHostSplitDnsPolicy,
) {
    let mut variables = Vec::new();
    if environment.doh_mode.is_some() {
        variables.push("THOR_DOH_MODE".to_string());
    }
    if environment.doh_provider.is_some() {
        variables.push("THOR_DOH_PROVIDER".to_string());
    }
    if environment.doh_template.is_some() {
        variables.push("THOR_DOH_TEMPLATE".to_string());
    }
    let source = if variables.is_empty() {
        DohPolicySource::ThorDefault
    } else {
        DohPolicySource::Environment { variables }
    };
    let mode_is_valid = environment
        .doh_mode
        .as_deref()
        .is_none_or(|value| matches!(value.trim(), "off" | "automatic" | "secure"));
    let mut mode = SecureDnsMode::from_policy_value(environment.doh_mode.as_deref());
    if mode == SecureDnsMode::Off {
        return (mode, None, source, DevHostSplitDnsPolicy::Disabled);
    }
    let custom = environment
        .doh_template
        .as_deref()
        .map(|template| DohProvider::custom(template.to_string()));
    let mut provider_input_valid = mode_is_valid;
    let provider = match environment
        .doh_provider
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("cloudflare") => DohProvider::Cloudflare,
        Some("quad9") => DohProvider::Quad9,
        Some("custom") => match custom {
            Some(Ok(provider)) => provider,
            _ => {
                provider_input_valid = false;
                DohProvider::Cloudflare
            }
        },
        Some(_) => {
            provider_input_valid = false;
            DohProvider::Cloudflare
        }
        None => match custom {
            Some(Ok(provider)) => provider,
            Some(Err(_)) => {
                provider_input_valid = false;
                DohProvider::Cloudflare
            }
            None => DohProvider::Cloudflare,
        },
    }
    .validated_or_default();
    if !provider_input_valid {
        mode = SecureDnsMode::Secure;
    }
    (
        mode,
        Some(provider),
        source,
        DevHostSplitDnsPolicy::Disabled,
    )
}

pub fn validate_doh_template(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 2048 || value.trim() != value {
        return Err("DoH template must be non-empty, at most 2048 bytes, and unpadded".into());
    }
    if !value.is_ascii()
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
        || value.contains('#')
        || value.contains('\\')
    {
        return Err("DoH template contains unsupported characters or a fragment".into());
    }
    let without_uri_template = value.strip_suffix("{?dns}").unwrap_or(value);
    if without_uri_template.contains('{') || without_uri_template.contains('}') {
        return Err("DoH template has an unsupported URI-template expression".into());
    }
    let rest = without_uri_template
        .strip_prefix("https://")
        .ok_or("DoH template must use https://")?;
    let authority_end = rest.find(['/', '?']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let path = &rest[authority_end..];
    if authority.is_empty() || authority.contains('@') || !path.starts_with('/') {
        return Err("DoH template requires a credential-free host and absolute path".into());
    }
    validate_doh_authority(authority)
}

fn validate_doh_authority(authority: &str) -> Result<(), String> {
    let (host, port) = if authority.starts_with('[') {
        let close = authority
            .find(']')
            .ok_or("DoH IPv6 host is missing closing bracket")?;
        let host = &authority[1..close];
        let suffix = &authority[close + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            Some(
                suffix
                    .strip_prefix(':')
                    .ok_or("invalid DoH authority suffix")?,
            )
        };
        if host.parse::<std::net::Ipv6Addr>().is_err() {
            return Err("invalid DoH IPv6 host".into());
        }
        (host, port)
    } else {
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => (host, Some(port)),
            _ => (authority, None),
        };
        if host.is_empty() || host.len() > 253 || host.starts_with('.') || host.ends_with('.') {
            return Err("invalid DoH host".into());
        }
        for label in host.split('.') {
            if label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
            {
                return Err("invalid DoH host label".into());
            }
        }
        (host, port)
    };
    let _ = host;
    if let Some(port) = port {
        let port = port
            .parse::<u16>()
            .map_err(|_| "DoH port must be between 1 and 65535")?;
        if port == 0 {
            return Err("DoH port must be between 1 and 65535".into());
        }
    }
    Ok(())
}

fn normalize_split_dns_suffixes(values: &[String]) -> Result<Vec<String>, String> {
    if values.len() > 32 {
        return Err("DevHost split DNS supports at most 32 suffixes".into());
    }
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        if value.trim() != value {
            return Err(format!("invalid DevHost split-DNS suffix: {value:?}"));
        }
        let raw_suffix = value.strip_prefix('.').unwrap_or(value);
        let suffix = raw_suffix.to_ascii_lowercase();
        if suffix.is_empty() || suffix.len() > 253 || suffix != raw_suffix {
            return Err(format!("invalid DevHost split-DNS suffix: {value:?}"));
        }
        for label in suffix.split('.') {
            if label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
            {
                return Err(format!("invalid DevHost split-DNS suffix: {value:?}"));
            }
        }
        normalized.push(suffix);
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

impl Default for PrivacySearchEngine {
    fn default() -> Self {
        Self::DuckDuckGo
    }
}

impl PrivacySearchEngine {
    pub fn search_url_template(&self) -> &'static str {
        match self {
            Self::DuckDuckGo => "https://duckduckgo.com/?q={}",
            Self::BraveSearch => "https://search.brave.com/search?q={}",
        }
    }

    pub fn build_search_url(&self, raw_query: &str) -> String {
        let encoded = urlencoding_simple(&ThorBrowserDefaults::sanitize_search_query(raw_query));
        self.search_url_template().replace("{}", &encoded)
    }
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Thor-owned defaults that are safe to pass to a Chromium process.
#[derive(Debug, Clone)]
pub struct ThorBrowserDefaults {
    pub search_engine: PrivacySearchEngine,
    pub https_first_enabled: bool,
    pub webrtc_ip_policy: WebRtcIpPolicy,
    pub secure_dns_mode: SecureDnsMode,
    pub secure_dns_template: String,
}

impl Default for ThorBrowserDefaults {
    fn default() -> Self {
        Self::new()
    }
}

impl ThorBrowserDefaults {
    pub fn new() -> Self {
        Self {
            search_engine: PrivacySearchEngine::DuckDuckGo,
            https_first_enabled: true,
            webrtc_ip_policy: WebRtcIpPolicy::DisableNonProxiedUdp,
            secure_dns_mode: SecureDnsMode::Secure,
            secure_dns_template: "https://cloudflare-dns.com/dns-query".into(),
        }
    }

    /// Stable Chromium switches for Thor's network/privacy baseline.
    pub fn chromium_runtime_flags() -> &'static [&'static str] {
        &["--no-default-browser-check", "--no-pings"]
    }

    /// Browser command line, including strict DoH. Operators can opt out or
    /// point Thor to a managed resolver with `THOR_DOH_MODE` / `THOR_DOH_TEMPLATE`.
    pub fn chromium_runtime_args() -> Vec<String> {
        Self::chromium_runtime_args_with_managed(None)
    }

    pub fn chromium_runtime_args_with_managed(
        managed_policy: Option<&ManagedDohPolicy>,
    ) -> Vec<String> {
        let mut args = Self::chromium_runtime_flags()
            .iter()
            .map(|flag| (*flag).to_string())
            .collect::<Vec<_>>();
        args.extend(Self::network_privacy_policy_with_managed(managed_policy).chromium_args());
        args
    }

    /// Typed view of the effective command-line network privacy policy.
    pub fn network_privacy_policy() -> NetworkPrivacyPolicy {
        NetworkPrivacyPolicy::from_environment()
    }

    pub fn network_privacy_policy_with_managed(
        managed_policy: Option<&ManagedDohPolicy>,
    ) -> NetworkPrivacyPolicy {
        NetworkPrivacyPolicy::resolve(managed_policy, &NetworkPrivacyEnvironment::from_process())
    }

    pub fn default_webrtc_ip_handling_policy() -> WebRtcIpPolicy {
        WebRtcIpPolicy::DisableNonProxiedUdp
    }

    /// Preserve explicit search syntax; the Chromium omnibox owns interpretation.
    pub fn sanitize_search_query(raw_query: &str) -> String {
        raw_query.trim().to_string()
    }

    pub fn build_search_url(&self, query: &str) -> String {
        self.search_engine.build_search_url(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chromium_runtime_flags_are_privacy_oriented() {
        let flags = ThorBrowserDefaults::chromium_runtime_flags();
        assert!(!flags.contains(&"--disable-background-networking"));
        assert!(flags.contains(&"--no-default-browser-check"));
        let policy = NetworkPrivacyPolicy::from_values(None, None, None);
        assert!(policy
            .chromium_args()
            .contains(&"--force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_string()));
        assert_eq!(
            ThorBrowserDefaults::new().webrtc_ip_policy,
            WebRtcIpPolicy::DisableNonProxiedUdp
        );
    }

    #[test]
    fn default_runtime_args_use_strict_doh() {
        let args = NetworkPrivacyPolicy::from_values(None, None, None).chromium_args();
        assert!(args.contains(&"--dns-over-https-mode=secure".to_string()));
        assert!(args
            .iter()
            .any(|arg| arg.starts_with("--dns-over-https-templates=")));
    }

    #[test]
    fn invalid_or_insecure_policy_values_fail_private() {
        let policy = NetworkPrivacyPolicy::from_values(
            Some("typo"),
            Some("http://resolver.invalid/dns-query"),
            Some("default"),
        );
        assert_eq!(policy.secure_dns_mode, SecureDnsMode::Secure);
        assert_eq!(
            policy.secure_dns_template.as_deref(),
            Some(NetworkPrivacyPolicy::DEFAULT_DOH_TEMPLATE)
        );
        assert_eq!(
            policy.webrtc_ip_policy,
            WebRtcIpPolicy::DisableNonProxiedUdp
        );

        let automatic_with_invalid_template = NetworkPrivacyPolicy::resolve(
            None,
            &NetworkPrivacyEnvironment {
                doh_mode: Some("automatic".into()),
                doh_template: Some("http://resolver.invalid/dns-query".into()),
                ..NetworkPrivacyEnvironment::default()
            },
        );
        assert_eq!(
            automatic_with_invalid_template.secure_dns_mode,
            SecureDnsMode::Secure
        );
        assert_eq!(
            automatic_with_invalid_template.secure_dns_provider,
            Some(DohProvider::Cloudflare)
        );
    }

    #[test]
    fn explicit_off_omits_doh_template() {
        let policy = NetworkPrivacyPolicy::from_values(
            Some("off"),
            Some("https://resolver.example/dns-query"),
            Some("disable_non_proxied_udp"),
        );
        assert_eq!(policy.secure_dns_template, None);
        assert_eq!(
            policy.webrtc_ip_policy,
            WebRtcIpPolicy::DisableNonProxiedUdp
        );
        assert!(!policy
            .chromium_args()
            .iter()
            .any(|argument| argument.starts_with("--dns-over-https-templates=")));
    }

    #[test]
    fn providers_and_custom_templates_are_strictly_validated() {
        assert!(validate_doh_template(CLOUDFLARE_DOH_TEMPLATE).is_ok());
        assert!(validate_doh_template(QUAD9_DOH_TEMPLATE).is_ok());
        assert!(DohProvider::custom("https://resolver.example/dns-query{?dns}").is_ok());
        for invalid in [
            "http://resolver.example/dns-query",
            "https://user@resolver.example/dns-query",
            "https://resolver.example",
            "https://resolver.example/dns-query#fragment",
            "https://resolver.example/dns query",
            " https://resolver.example/dns-query",
            "https://.example/dns-query",
            "https://resolver.example:0/dns-query",
            "https://resolver.example/dns-query{dns}",
        ] {
            assert!(
                validate_doh_template(invalid).is_err(),
                "accepted invalid template {invalid:?}"
            );
        }
    }

    #[test]
    fn environment_selects_quad9_or_backward_compatible_custom_template() {
        let quad9 = NetworkPrivacyPolicy::resolve(
            None,
            &NetworkPrivacyEnvironment {
                doh_provider: Some("quad9".into()),
                ..NetworkPrivacyEnvironment::default()
            },
        );
        assert_eq!(quad9.secure_dns_provider, Some(DohProvider::Quad9));
        assert_eq!(
            quad9.secure_dns_source,
            DohPolicySource::Environment {
                variables: vec!["THOR_DOH_PROVIDER".into()]
            }
        );

        let custom_template = "https://doh.example.test/query{?dns}";
        let custom = NetworkPrivacyPolicy::resolve(
            None,
            &NetworkPrivacyEnvironment {
                doh_template: Some(custom_template.into()),
                ..NetworkPrivacyEnvironment::default()
            },
        );
        assert_eq!(custom.secure_dns_template.as_deref(), Some(custom_template));
        assert!(matches!(
            custom.secure_dns_provider,
            Some(DohProvider::Custom { .. })
        ));
    }

    #[test]
    fn managed_doh_precedes_environment_and_split_dns_is_devhost_only() {
        let managed = ManagedDohPolicy::new(
            "enterprise-security-v3",
            SecureDnsMode::Secure,
            DohProvider::Quad9,
        )
        .unwrap()
        .with_devhost_split_dns(vec!["corp.example".into(), ".internal.test".into()])
        .unwrap();
        let environment = NetworkPrivacyEnvironment {
            doh_mode: Some("off".into()),
            doh_provider: Some("cloudflare".into()),
            doh_template: Some("http://invalid.example/dns-query".into()),
            webrtc_ip_policy: Some("default_public_interface_only".into()),
        };
        let policy = NetworkPrivacyPolicy::resolve(Some(&managed), &environment);
        assert_eq!(policy.secure_dns_mode, SecureDnsMode::Secure);
        assert_eq!(policy.secure_dns_provider, Some(DohProvider::Quad9));
        assert_eq!(
            policy.secure_dns_source,
            DohPolicySource::ManagedPolicy {
                policy_id: "enterprise-security-v3".into()
            }
        );
        // Managed DoH does not silently claim ownership of the separate WebRTC setting.
        assert_eq!(
            policy.webrtc_ip_policy,
            WebRtcIpPolicy::DefaultPublicInterfaceOnly
        );
        assert!(matches!(
            policy.split_dns_readback(ProfileMode::DevHost),
            SplitDnsReadback::DevHostOnly { .. }
        ));
        assert!(matches!(
            policy.split_dns_readback(ProfileMode::StealthTarget),
            SplitDnsReadback::SuppressedForTargetProfile {
                profile: ProfileMode::StealthTarget,
                ..
            }
        ));
        assert!(matches!(
            policy.split_dns_readback(ProfileMode::HardenedTarget),
            SplitDnsReadback::SuppressedForTargetProfile {
                profile: ProfileMode::HardenedTarget,
                ..
            }
        ));
    }

    #[test]
    fn invalid_managed_input_fails_to_strict_default_without_env_fallback() {
        let invalid = ManagedDohPolicy {
            policy_id: "enterprise-invalid".into(),
            mode: SecureDnsMode::Automatic,
            provider: DohProvider::Custom {
                https_template: "http://resolver.invalid/dns-query".into(),
            },
            devhost_split_dns_suffixes: vec!["*.corp.example".into()],
        };
        let environment = NetworkPrivacyEnvironment {
            doh_mode: Some("off".into()),
            ..NetworkPrivacyEnvironment::default()
        };
        let policy = NetworkPrivacyPolicy::resolve(Some(&invalid), &environment);
        assert_eq!(policy.secure_dns_mode, SecureDnsMode::Secure);
        assert_eq!(policy.secure_dns_provider, Some(DohProvider::Cloudflare));
        assert_eq!(policy.devhost_split_dns, DevHostSplitDnsPolicy::Disabled);
        assert!(matches!(
            policy.secure_dns_source,
            DohPolicySource::ManagedPolicy { .. }
        ));
    }

    #[test]
    fn health_is_unchecked_until_matching_verified_probe_is_recorded() {
        let mut policy = NetworkPrivacyPolicy::from_values(None, None, None);
        let _args = policy.chromium_args();
        assert_eq!(
            policy.doh_health_readback().state,
            DohHealthState::Unchecked
        );
        policy
            .record_verified_doh_probe(DohProbeResult::Healthy {
                template: CLOUDFLARE_DOH_TEMPLATE.into(),
            })
            .unwrap();
        assert_eq!(policy.doh_health_readback().state, DohHealthState::Healthy);
        assert!(policy.doh_health_readback().checked_at_unix_ms.is_some());
        let serialized = serde_json::to_string(&policy).unwrap();
        let restored: NetworkPrivacyPolicy = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            restored.doh_health_readback().state,
            DohHealthState::Unchecked,
            "health must be freshly verified after policy restore"
        );
        let mut stale = policy.clone();
        stale.secure_dns_template = Some(QUAD9_DOH_TEMPLATE.into());
        assert_eq!(stale.doh_health_readback().state, DohHealthState::Degraded);

        let error = policy
            .record_verified_doh_probe(DohProbeResult::Healthy {
                template: QUAD9_DOH_TEMPLATE.into(),
            })
            .unwrap_err();
        assert!(error.contains("does not match"));
        assert_eq!(policy.doh_health_readback().state, DohHealthState::Degraded);
    }

    #[test]
    fn webrtc_health_requires_matching_browser_observation_and_is_not_restored() {
        let mut policy = NetworkPrivacyPolicy::from_values(None, None, None);
        let _args = policy.chromium_args();
        assert_eq!(
            policy.webrtc_health_readback().state,
            WebRtcHealthState::Unchecked
        );

        policy
            .record_verified_webrtc_probe(WebRtcProbeResult::Observed {
                policy: WebRtcIpPolicy::DisableNonProxiedUdp,
            })
            .unwrap();
        assert_eq!(
            policy.webrtc_health_readback().state,
            WebRtcHealthState::Healthy
        );

        let serialized = serde_json::to_string(&policy).unwrap();
        let restored: NetworkPrivacyPolicy = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            restored.webrtc_health_readback().state,
            WebRtcHealthState::Unchecked
        );

        let error = policy
            .record_verified_webrtc_probe(WebRtcProbeResult::Observed {
                policy: WebRtcIpPolicy::DefaultPublicInterfaceOnly,
            })
            .unwrap_err();
        assert!(error.contains("does not match"));
        assert_eq!(
            policy.webrtc_health_readback().state,
            WebRtcHealthState::Degraded
        );
    }

    #[test]
    fn search_syntax_is_preserved() {
        assert_eq!(
            ThorBrowserDefaults::sanitize_search_query(" !wiki Rust "),
            "!wiki Rust"
        );
        assert!(PrivacySearchEngine::DuckDuckGo
            .build_search_url("!wiki Rust")
            .contains("%21wiki"));
    }
}
