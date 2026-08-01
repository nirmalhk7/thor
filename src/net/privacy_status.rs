//! Effective privacy readback assembled from validated runtime evidence.
//!
//! Requested policy and launcher arguments are never promoted to healthy
//! enforcement. Browser-backed probes must be recorded explicitly, while the
//! bundled request filter is cryptographically revalidated during collection.

use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::browser_defaults::{
    validate_doh_template, DohHealthState, DohPolicySource, DohProvider, NetworkPrivacyPolicy,
    SecureDnsMode, WebRtcHealthState, WebRtcIpPolicy,
};

use super::security_shield::{SecurityShield, VerifiedFilterBundle};

/// A healthy observation remains current for five minutes. Long-lived callers
/// must repeat native/browser probes instead of treating startup state as
/// permanent enforcement.
pub const MAX_PRIVACY_READBACK_AGE_MS: u128 = 5 * 60 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectivePrivacyState {
    Disabled,
    Unchecked,
    Healthy,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveControlReadback {
    pub state: EffectivePrivacyState,
    pub checked_at_unix_ms: Option<u128>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveDohStatus {
    pub mode: SecureDnsMode,
    pub provider: Option<DohProvider>,
    pub template: Option<String>,
    pub source: DohPolicySource,
    pub readback: EffectiveControlReadback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveWebRtcStatus {
    pub requested_policy: WebRtcIpPolicy,
    pub readback: EffectiveControlReadback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveFilterBundleStatus {
    pub readback: EffectiveControlReadback,
    pub verified_bundle: Option<VerifiedFilterBundle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveSecurityServiceStatus {
    pub service: NativeSecurityService,
    pub readback: EffectiveControlReadback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectivePrivacyStatus {
    pub secure_dns: EffectiveDohStatus,
    pub webrtc_ip_policy: EffectiveWebRtcStatus,
    pub request_filter_bundle: EffectiveFilterBundleStatus,
    pub security_service: EffectiveSecurityServiceStatus,
    pub component_updates: EffectiveSecurityServiceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeSecurityService {
    SafeBrowsing,
    ComponentUpdates,
}

/// Browser observation of the enabled filter ruleset. A healthy result is
/// derived only after this observation matches a freshly verified bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterRuntimeProbeResult {
    Observed {
        enabled: bool,
        bundle_version: String,
        rule_count: usize,
        enforcement: String,
    },
    ProbeFailed {
        reason: String,
    },
}

/// Browser/native observation of a security service. The aggregator decides
/// health from `enabled && ready`; callers must not use configuration intent as
/// this observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityServiceProbeResult {
    Observed {
        enabled: bool,
        ready: bool,
        evidence: String,
    },
    ProbeFailed {
        reason: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct PrivacyRuntimeReadbacks {
    filter_bundle: Option<TimedFilterReadback>,
    safe_browsing: Option<TimedServiceReadback>,
    component_updates: Option<TimedServiceReadback>,
}

#[derive(Debug, Clone)]
struct TimedFilterReadback {
    checked_at_unix_ms: u128,
    result: FilterRuntimeProbeResult,
}

#[derive(Debug, Clone)]
struct TimedServiceReadback {
    checked_at_unix_ms: u128,
    result: SecurityServiceProbeResult,
}

impl PrivacyRuntimeReadbacks {
    pub fn record_filter_bundle_probe(
        &mut self,
        result: FilterRuntimeProbeResult,
    ) -> Result<(), String> {
        self.filter_bundle = Some(TimedFilterReadback {
            checked_at_unix_ms: unix_time_ms("filter bundle probe")?,
            result,
        });
        Ok(())
    }

    pub fn record_security_service_probe(
        &mut self,
        service: NativeSecurityService,
        result: SecurityServiceProbeResult,
    ) -> Result<(), String> {
        let readback = TimedServiceReadback {
            checked_at_unix_ms: unix_time_ms("security service probe")?,
            result,
        };
        match service {
            NativeSecurityService::SafeBrowsing => self.safe_browsing = Some(readback),
            NativeSecurityService::ComponentUpdates => self.component_updates = Some(readback),
        }
        Ok(())
    }
}

/// Build effective status. This performs file-system and signature validation
/// of `extension_dir` every time; it never infers runtime filtering, WebRTC,
/// Safe Browsing, or component-update health from configured defaults.
pub fn effective_privacy_status(
    policy: &NetworkPrivacyPolicy,
    extension_dir: &Path,
    runtime: &PrivacyRuntimeReadbacks,
) -> EffectivePrivacyStatus {
    let now = unix_time_ms("privacy status collection").ok();
    effective_privacy_status_at(policy, extension_dir, runtime, now)
}

fn effective_privacy_status_at(
    policy: &NetworkPrivacyPolicy,
    extension_dir: &Path,
    runtime: &PrivacyRuntimeReadbacks,
    now: Option<u128>,
) -> EffectivePrivacyStatus {
    EffectivePrivacyStatus {
        secure_dns: effective_doh_status(policy, now),
        webrtc_ip_policy: effective_webrtc_status(policy, now),
        request_filter_bundle: effective_filter_status(extension_dir, runtime, now),
        security_service: effective_service_status(
            NativeSecurityService::SafeBrowsing,
            runtime.safe_browsing.as_ref(),
            now,
        ),
        component_updates: effective_service_status(
            NativeSecurityService::ComponentUpdates,
            runtime.component_updates.as_ref(),
            now,
        ),
    }
}

fn effective_doh_status(policy: &NetworkPrivacyPolicy, now: Option<u128>) -> EffectiveDohStatus {
    let configuration_error = validate_effective_doh_configuration(policy).err();
    let health = policy.doh_health_readback();
    let readback = if let Some(error) = configuration_error {
        degraded(
            None,
            format!("invalid effective DoH configuration: {error}"),
        )
    } else if policy.secure_dns_mode == SecureDnsMode::Off {
        disabled("secure DNS is explicitly disabled")
    } else {
        match health.state {
            DohHealthState::Unchecked => unchecked("DoH enforcement has not been probed"),
            DohHealthState::Healthy => {
                checked_health(health.checked_at_unix_ms, health.detail, now, "DoH probe")
            }
            DohHealthState::Degraded => degraded(
                health.checked_at_unix_ms,
                health
                    .detail
                    .unwrap_or_else(|| "DoH probe reported degraded".into()),
            ),
        }
    };
    EffectiveDohStatus {
        mode: policy.secure_dns_mode,
        provider: policy.secure_dns_provider.clone(),
        template: policy.secure_dns_template.clone(),
        source: policy.secure_dns_source.clone(),
        readback,
    }
}

fn validate_effective_doh_configuration(policy: &NetworkPrivacyPolicy) -> Result<(), String> {
    match policy.secure_dns_mode {
        SecureDnsMode::Off => {
            if policy.secure_dns_template.is_some() || policy.secure_dns_provider.is_some() {
                return Err("disabled mode retains a resolver or template".into());
            }
        }
        SecureDnsMode::Automatic | SecureDnsMode::Secure => {
            let template = policy
                .secure_dns_template
                .as_deref()
                .ok_or("enabled mode has no resolver template")?;
            validate_doh_template(template)?;
            let provider = policy
                .secure_dns_provider
                .as_ref()
                .ok_or("enabled mode has no resolver provider")?;
            if provider.https_template() != template {
                return Err("resolver provider and template disagree".into());
            }
        }
    }
    match &policy.secure_dns_source {
        DohPolicySource::ThorDefault => {}
        DohPolicySource::Environment { variables } if variables.is_empty() => {
            return Err("environment provenance has no variables".into());
        }
        DohPolicySource::ManagedPolicy { policy_id } if policy_id.trim().is_empty() => {
            return Err("managed provenance has no policy ID".into());
        }
        DohPolicySource::Environment { .. } | DohPolicySource::ManagedPolicy { .. } => {}
    }
    Ok(())
}

fn effective_webrtc_status(
    policy: &NetworkPrivacyPolicy,
    now: Option<u128>,
) -> EffectiveWebRtcStatus {
    let health = policy.webrtc_health_readback();
    let readback = match health.state {
        WebRtcHealthState::Unchecked => {
            unchecked("WebRTC enforcement has not been observed in the running browser")
        }
        WebRtcHealthState::Healthy => checked_health(
            health.checked_at_unix_ms,
            health.detail,
            now,
            "WebRTC probe",
        ),
        WebRtcHealthState::Degraded => degraded(
            health.checked_at_unix_ms,
            health
                .detail
                .unwrap_or_else(|| "WebRTC probe reported degraded".into()),
        ),
    };
    EffectiveWebRtcStatus {
        requested_policy: policy.webrtc_ip_policy,
        readback,
    }
}

fn effective_filter_status(
    extension_dir: &Path,
    runtime: &PrivacyRuntimeReadbacks,
    now: Option<u128>,
) -> EffectiveFilterBundleStatus {
    let verified_bundle = match SecurityShield::verify_filter_bundle(extension_dir) {
        Ok(bundle) => bundle,
        Err(error) => {
            return EffectiveFilterBundleStatus {
                readback: degraded(None, format!("signed filter bundle is invalid: {error}")),
                verified_bundle: None,
            };
        }
    };
    let readback = match runtime.filter_bundle.as_ref() {
        None => unchecked(
            "signed filter bundle is valid, but runtime ruleset enforcement is unobserved",
        ),
        Some(observation) if is_stale(observation.checked_at_unix_ms, now) => degraded(
            Some(observation.checked_at_unix_ms),
            "filter runtime observation is stale",
        ),
        Some(observation) => match &observation.result {
            FilterRuntimeProbeResult::Observed {
                enabled,
                bundle_version,
                rule_count,
                enforcement,
            } if *enabled
                && bundle_version == &verified_bundle.bundle_version
                && *rule_count == verified_bundle.rule_count
                && enforcement == verified_bundle.enforcement =>
            {
                healthy(Some(observation.checked_at_unix_ms), None)
            }
            FilterRuntimeProbeResult::Observed {
                enabled,
                bundle_version,
                rule_count,
                enforcement,
            } => degraded(
                Some(observation.checked_at_unix_ms),
                format!(
                    "runtime filter does not match verified bundle (enabled={enabled}, version={bundle_version:?}, rules={rule_count}, enforcement={enforcement:?})"
                ),
            ),
            FilterRuntimeProbeResult::ProbeFailed { reason } => degraded(
                Some(observation.checked_at_unix_ms),
                normalized_reason(reason, "filter runtime probe failed without detail"),
            ),
        },
    };
    EffectiveFilterBundleStatus {
        readback,
        verified_bundle: Some(verified_bundle),
    }
}

fn effective_service_status(
    service: NativeSecurityService,
    observation: Option<&TimedServiceReadback>,
    now: Option<u128>,
) -> EffectiveSecurityServiceStatus {
    let label = match service {
        NativeSecurityService::SafeBrowsing => "Safe Browsing",
        NativeSecurityService::ComponentUpdates => "component updates",
    };
    let readback = match observation {
        None => unchecked(format!("{label} readiness has not been observed")),
        Some(observation) if is_stale(observation.checked_at_unix_ms, now) => degraded(
            Some(observation.checked_at_unix_ms),
            format!("{label} observation is stale"),
        ),
        Some(observation) => match &observation.result {
            SecurityServiceProbeResult::Observed {
                enabled: true,
                ready: true,
                evidence,
            } if !evidence.trim().is_empty() => {
                healthy(Some(observation.checked_at_unix_ms), Some(evidence.clone()))
            }
            SecurityServiceProbeResult::Observed {
                enabled,
                ready,
                evidence,
            } => degraded(
                Some(observation.checked_at_unix_ms),
                format!(
                    "{label} is not ready (enabled={enabled}, ready={ready}, evidence={evidence:?})"
                ),
            ),
            SecurityServiceProbeResult::ProbeFailed { reason } => degraded(
                Some(observation.checked_at_unix_ms),
                normalized_reason(reason, &format!("{label} probe failed without detail")),
            ),
        },
    };
    EffectiveSecurityServiceStatus { service, readback }
}

fn checked_health(
    checked_at_unix_ms: Option<u128>,
    detail: Option<String>,
    now: Option<u128>,
    label: &str,
) -> EffectiveControlReadback {
    let Some(checked_at) = checked_at_unix_ms else {
        return degraded(None, format!("{label} has no observation timestamp"));
    };
    if is_stale(checked_at, now) {
        return degraded(Some(checked_at), format!("{label} observation is stale"));
    }
    healthy(Some(checked_at), detail)
}

fn is_stale(checked_at_unix_ms: u128, now: Option<u128>) -> bool {
    match now {
        Some(now) => now
            .checked_sub(checked_at_unix_ms)
            .is_none_or(|age| age > MAX_PRIVACY_READBACK_AGE_MS),
        None => true,
    }
}

fn healthy(checked_at_unix_ms: Option<u128>, detail: Option<String>) -> EffectiveControlReadback {
    EffectiveControlReadback {
        state: EffectivePrivacyState::Healthy,
        checked_at_unix_ms,
        detail,
    }
}

fn degraded(
    checked_at_unix_ms: Option<u128>,
    detail: impl Into<String>,
) -> EffectiveControlReadback {
    EffectiveControlReadback {
        state: EffectivePrivacyState::Degraded,
        checked_at_unix_ms,
        detail: Some(detail.into()),
    }
}

fn unchecked(detail: impl Into<String>) -> EffectiveControlReadback {
    EffectiveControlReadback {
        state: EffectivePrivacyState::Unchecked,
        checked_at_unix_ms: None,
        detail: Some(detail.into()),
    }
}

fn disabled(detail: impl Into<String>) -> EffectiveControlReadback {
    EffectiveControlReadback {
        state: EffectivePrivacyState::Disabled,
        checked_at_unix_ms: None,
        detail: Some(detail.into()),
    }
}

fn normalized_reason(reason: &str, fallback: &str) -> String {
    if reason.trim().is_empty() {
        fallback.to_string()
    } else {
        reason.to_string()
    }
}

fn unix_time_ms(operation: &str) -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| format!("read wall clock for {operation}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::browser_defaults::{
        DohProbeResult, ManagedDohPolicy, NetworkPrivacyEnvironment, WebRtcProbeResult,
        CLOUDFLARE_DOH_TEMPLATE,
    };
    use std::path::PathBuf;

    fn extension_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("browser/extensions/thor-shield")
    }

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "thor-privacy-status-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn strict_policy() -> NetworkPrivacyPolicy {
        NetworkPrivacyPolicy::resolve(None, &NetworkPrivacyEnvironment::default())
    }

    #[test]
    fn configured_controls_and_valid_bundle_remain_unchecked_without_runtime_readback() {
        let policy = strict_policy();
        let status = effective_privacy_status(
            &policy,
            &extension_dir(),
            &PrivacyRuntimeReadbacks::default(),
        );

        assert_eq!(
            status.secure_dns.readback.state,
            EffectivePrivacyState::Unchecked
        );
        assert_eq!(
            status.webrtc_ip_policy.readback.state,
            EffectivePrivacyState::Unchecked
        );
        assert_eq!(
            status.request_filter_bundle.readback.state,
            EffectivePrivacyState::Unchecked
        );
        assert!(status.request_filter_bundle.verified_bundle.is_some());
        assert_eq!(
            status.security_service.readback.state,
            EffectivePrivacyState::Unchecked
        );
        assert_eq!(
            status.component_updates.readback.state,
            EffectivePrivacyState::Unchecked
        );
    }

    #[test]
    fn managed_source_is_reported_without_assuming_health() {
        let managed = ManagedDohPolicy::new(
            "security-team-v4",
            SecureDnsMode::Secure,
            DohProvider::Cloudflare,
        )
        .unwrap();
        let policy =
            NetworkPrivacyPolicy::resolve(Some(&managed), &NetworkPrivacyEnvironment::default());
        let status = effective_privacy_status(
            &policy,
            &extension_dir(),
            &PrivacyRuntimeReadbacks::default(),
        );

        assert_eq!(
            status.secure_dns.source,
            DohPolicySource::ManagedPolicy {
                policy_id: "security-team-v4".into()
            }
        );
        assert_eq!(
            status.secure_dns.readback.state,
            EffectivePrivacyState::Unchecked
        );
    }

    #[test]
    fn matching_observations_produce_healthy_effective_status() {
        let mut policy = strict_policy();
        policy
            .record_verified_doh_probe(DohProbeResult::Healthy {
                template: CLOUDFLARE_DOH_TEMPLATE.into(),
            })
            .unwrap();
        policy
            .record_verified_webrtc_probe(WebRtcProbeResult::Observed {
                policy: WebRtcIpPolicy::DisableNonProxiedUdp,
            })
            .unwrap();

        let verified = SecurityShield::verify_filter_bundle(&extension_dir()).unwrap();
        let mut runtime = PrivacyRuntimeReadbacks::default();
        runtime
            .record_filter_bundle_probe(FilterRuntimeProbeResult::Observed {
                enabled: true,
                bundle_version: verified.bundle_version.clone(),
                rule_count: verified.rule_count,
                enforcement: verified.enforcement.into(),
            })
            .unwrap();
        for service in [
            NativeSecurityService::SafeBrowsing,
            NativeSecurityService::ComponentUpdates,
        ] {
            runtime
                .record_security_service_probe(
                    service,
                    SecurityServiceProbeResult::Observed {
                        enabled: true,
                        ready: true,
                        evidence: "native service status API".into(),
                    },
                )
                .unwrap();
        }

        let status = effective_privacy_status(&policy, &extension_dir(), &runtime);
        assert_eq!(
            status.secure_dns.readback.state,
            EffectivePrivacyState::Healthy
        );
        assert_eq!(
            status.webrtc_ip_policy.readback.state,
            EffectivePrivacyState::Healthy
        );
        assert_eq!(
            status.request_filter_bundle.readback.state,
            EffectivePrivacyState::Healthy
        );
        assert_eq!(
            status.security_service.readback.state,
            EffectivePrivacyState::Healthy
        );
        assert_eq!(
            status.component_updates.readback.state,
            EffectivePrivacyState::Healthy
        );
    }

    #[test]
    fn mismatched_runtime_observations_are_degraded() {
        let mut policy = strict_policy();
        let error = policy
            .record_verified_webrtc_probe(WebRtcProbeResult::Observed {
                policy: WebRtcIpPolicy::DefaultPublicInterfaceOnly,
            })
            .unwrap_err();
        assert!(error.contains("does not match"));

        let mut runtime = PrivacyRuntimeReadbacks::default();
        runtime
            .record_filter_bundle_probe(FilterRuntimeProbeResult::Observed {
                enabled: false,
                bundle_version: "wrong-version".into(),
                rule_count: 0,
                enforcement: "not-native".into(),
            })
            .unwrap();
        runtime
            .record_security_service_probe(
                NativeSecurityService::SafeBrowsing,
                SecurityServiceProbeResult::Observed {
                    enabled: true,
                    ready: false,
                    evidence: "database unavailable".into(),
                },
            )
            .unwrap();

        let status = effective_privacy_status(&policy, &extension_dir(), &runtime);
        assert_eq!(
            status.webrtc_ip_policy.readback.state,
            EffectivePrivacyState::Degraded
        );
        assert_eq!(
            status.request_filter_bundle.readback.state,
            EffectivePrivacyState::Degraded
        );
        assert_eq!(
            status.security_service.readback.state,
            EffectivePrivacyState::Degraded
        );
    }

    #[test]
    fn tampered_filter_bundle_is_degraded_even_with_matching_runtime_claim() {
        let temp_dir = test_dir("tampered-filter");
        std::fs::copy(
            extension_dir().join("filter_bundle.json"),
            temp_dir.join("filter_bundle.json"),
        )
        .unwrap();
        std::fs::write(temp_dir.join("rules.json"), b"[]").unwrap();
        let mut runtime = PrivacyRuntimeReadbacks::default();
        runtime
            .record_filter_bundle_probe(FilterRuntimeProbeResult::Observed {
                enabled: true,
                bundle_version: "2026.1.0".into(),
                rule_count: 1,
                enforcement: "chromium-extension-dnr-transitional".into(),
            })
            .unwrap();

        let status = effective_privacy_status(&strict_policy(), &temp_dir, &runtime);
        assert_eq!(
            status.request_filter_bundle.readback.state,
            EffectivePrivacyState::Degraded
        );
        assert!(status.request_filter_bundle.verified_bundle.is_none());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn stale_observations_are_degraded() {
        let mut runtime = PrivacyRuntimeReadbacks::default();
        runtime.safe_browsing = Some(TimedServiceReadback {
            checked_at_unix_ms: 10,
            result: SecurityServiceProbeResult::Observed {
                enabled: true,
                ready: true,
                evidence: "native service status API".into(),
            },
        });
        let now = Some(10 + MAX_PRIVACY_READBACK_AGE_MS + 1);
        let status = effective_privacy_status_at(&strict_policy(), &extension_dir(), &runtime, now);
        assert_eq!(
            status.security_service.readback.state,
            EffectivePrivacyState::Degraded
        );
    }
}
