use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Browser Profile Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileMode {
    /// Host / Dev Profile: Full WebGL, permission-gated developer APIs, IDE bridge enabled, no fingerprint noise
    DevHost,
    /// Stealth Target Profile: native anti-tracking target with strict GPC/DNT headers;
    /// script-level Canvas/Audio perturbation is intentionally not used.
    StealthTarget,
    /// Hardened Target Profile: requires a disposable Chromium BrowserContext
    /// and disables optional developer-facing surface area.
    HardenedTarget,
}

impl ProfileMode {
    /// Stable value consumed by Thor's owned-Chromium profile-policy patch.
    pub const fn chromium_value(self) -> &'static str {
        match self {
            Self::DevHost => "devhost",
            Self::StealthTarget => "stealth",
            Self::HardenedTarget => "hardened",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThirdPartyCookiePolicy {
    Allow,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebRtcExposurePolicy {
    Default,
    PublicInterfaceOnly,
    DisableNonProxiedUdp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecureDnsPolicy {
    System,
    Automatic,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JitPolicy {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DangerousApiPolicy {
    ChromiumDefault,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadPolicy {
    ChromiumDefault,
    RequireSafeBrowsing,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfacePolicy {
    Enabled,
    Disabled,
    Restricted,
}

/// Privacy behavior requested by a Thor profile.
///
/// Chromium registry defaults are preferences, not immutable policy: an
/// existing user pref or enterprise policy can override them. Callers must use
/// [`PrivacyStatus`] when reporting actual enforcement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub https_only: bool,
    pub secure_dns: SecureDnsPolicy,
    pub save_browser_history: bool,
    pub third_party_cookies: ThirdPartyCookiePolicy,
    pub related_website_sets: bool,
    pub privacy_sandbox_topics: bool,
    pub privacy_sandbox_protected_audience: bool,
    pub privacy_sandbox_ad_measurement: bool,
    pub search_suggestions: bool,
    pub translation_offers: bool,
    pub do_not_track: bool,
    pub global_privacy_control: bool,
    pub network_prediction: bool,
    pub alternate_error_pages: bool,
    pub hyperlink_auditing: bool,
    pub strict_origin_isolation: bool,
    pub webrtc_ip_policy: WebRtcExposurePolicy,
    pub jit: JitPolicy,
    pub dangerous_apis: DangerousApiPolicy,
    pub downloads: DownloadPolicy,
    pub webgl: SurfacePolicy,
    pub webgpu: SurfacePolicy,
    pub media: SurfacePolicy,
    pub require_isolated_identity: bool,
}

impl PrivacyPolicy {
    pub fn thor_native_defaults() -> Self {
        Self {
            https_only: true,
            secure_dns: SecureDnsPolicy::Strict,
            save_browser_history: false,
            third_party_cookies: ThirdPartyCookiePolicy::Block,
            related_website_sets: false,
            privacy_sandbox_topics: false,
            privacy_sandbox_protected_audience: false,
            privacy_sandbox_ad_measurement: false,
            search_suggestions: false,
            translation_offers: false,
            do_not_track: true,
            global_privacy_control: true,
            network_prediction: false,
            alternate_error_pages: false,
            hyperlink_auditing: false,
            strict_origin_isolation: true,
            webrtc_ip_policy: WebRtcExposurePolicy::PublicInterfaceOnly,
            jit: JitPolicy::Enabled,
            dangerous_apis: DangerousApiPolicy::ChromiumDefault,
            downloads: DownloadPolicy::ChromiumDefault,
            webgl: SurfacePolicy::Enabled,
            webgpu: SurfacePolicy::Enabled,
            media: SurfacePolicy::Enabled,
            require_isolated_identity: false,
        }
    }

    pub fn stealth_target() -> Self {
        Self {
            webrtc_ip_policy: WebRtcExposurePolicy::DisableNonProxiedUdp,
            ..Self::thor_native_defaults()
        }
    }

    pub fn hardened_target() -> Self {
        Self {
            webrtc_ip_policy: WebRtcExposurePolicy::DisableNonProxiedUdp,
            jit: JitPolicy::Disabled,
            dangerous_apis: DangerousApiPolicy::Restricted,
            downloads: DownloadPolicy::RequireSafeBrowsing,
            webgl: SurfacePolicy::Disabled,
            webgpu: SurfacePolicy::Disabled,
            media: SurfacePolicy::Restricted,
            require_isolated_identity: true,
            ..Self::thor_native_defaults()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyControlStatus {
    NativeDefault,
    BrowserContext,
    BestEffort,
    Disabled,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyLimitation {
    NativeDefaultsNotVerified,
    ExistingPreferencesMayOverrideDefaults,
    IdentityContextRequired,
    FingerprintingProtectionIsBestEffort,
    SecureDnsNotVerified,
    GlobalPrivacyControlIsBestEffort,
    GlobalPrivacyControlNotVerified,
    PrivacySandboxApisNotVerified,
    WebRtcPolicyNotVerified,
    JitPolicyNotEnforced,
    DangerousApiPolicyNotEnforced,
    DownloadPolicyNotVerified,
    GraphicsPolicyNotEnforced,
    MediaPolicyNotEnforced,
    RequestFilterBundleNotVerified,
    SecurityServiceNotVerified,
    ComponentUpdatesNotVerified,
}

/// Typed runtime status. Separates requested policy from enforcement evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyStatus {
    pub policy: PrivacyPolicy,
    pub native_defaults: PrivacyControlStatus,
    pub identity_isolation: PrivacyControlStatus,
    pub fingerprint_protection: PrivacyControlStatus,
    pub secure_dns: PrivacyControlStatus,
    pub global_privacy_control: PrivacyControlStatus,
    pub privacy_sandbox_apis: PrivacyControlStatus,
    pub webrtc_ip_policy: PrivacyControlStatus,
    pub jit: PrivacyControlStatus,
    pub dangerous_apis: PrivacyControlStatus,
    pub downloads: PrivacyControlStatus,
    pub webgl: PrivacyControlStatus,
    pub webgpu: PrivacyControlStatus,
    pub media: PrivacyControlStatus,
    pub request_filter_bundle: PrivacyControlStatus,
    pub security_service: PrivacyControlStatus,
    pub component_updates: PrivacyControlStatus,
    pub limitations: Vec<PrivacyLimitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub mode: ProfileMode,
    pub privacy: PrivacyPolicy,
    pub strip_tracking_params: bool,
    pub inject_gpc_dnt_headers: bool,
    pub randomize_fingerprints: bool,
    pub allow_webgl: bool,
    pub enable_ide_bridge: bool,
}

impl ProfileConfig {
    pub fn dev_host_default() -> Self {
        Self {
            mode: ProfileMode::DevHost,
            privacy: PrivacyPolicy::thor_native_defaults(),
            strip_tracking_params: false,
            inject_gpc_dnt_headers: false,
            randomize_fingerprints: false,
            allow_webgl: true,
            enable_ide_bridge: true,
        }
    }

    pub fn stealth_target_default() -> Self {
        Self {
            mode: ProfileMode::StealthTarget,
            privacy: PrivacyPolicy::stealth_target(),
            strip_tracking_params: true,
            inject_gpc_dnt_headers: true,
            randomize_fingerprints: true,
            allow_webgl: true, // Enabled; fingerprint protection remains best effort.
            enable_ide_bridge: false,
        }
    }

    pub fn hardened_target_default() -> Self {
        Self {
            mode: ProfileMode::HardenedTarget,
            privacy: PrivacyPolicy::hardened_target(),
            strip_tracking_params: true,
            inject_gpc_dnt_headers: true,
            randomize_fingerprints: true,
            allow_webgl: false,
            enable_ide_bridge: false,
        }
    }

    pub fn privacy_status(
        &self,
        native_defaults_verified: bool,
        identity_isolated: bool,
    ) -> PrivacyStatus {
        let mut limitations = vec![PrivacyLimitation::ExistingPreferencesMayOverrideDefaults];
        if !native_defaults_verified {
            limitations.push(PrivacyLimitation::NativeDefaultsNotVerified);
        }
        if self.privacy.require_isolated_identity && !identity_isolated {
            limitations.push(PrivacyLimitation::IdentityContextRequired);
        }
        if self.randomize_fingerprints {
            limitations.push(PrivacyLimitation::FingerprintingProtectionIsBestEffort);
        }
        limitations.extend([
            PrivacyLimitation::SecureDnsNotVerified,
            PrivacyLimitation::WebRtcPolicyNotVerified,
            PrivacyLimitation::JitPolicyNotEnforced,
            PrivacyLimitation::DangerousApiPolicyNotEnforced,
            PrivacyLimitation::DownloadPolicyNotVerified,
            PrivacyLimitation::GraphicsPolicyNotEnforced,
            PrivacyLimitation::MediaPolicyNotEnforced,
            PrivacyLimitation::RequestFilterBundleNotVerified,
            PrivacyLimitation::SecurityServiceNotVerified,
            PrivacyLimitation::ComponentUpdatesNotVerified,
        ]);
        if !native_defaults_verified {
            limitations.push(PrivacyLimitation::PrivacySandboxApisNotVerified);
        }
        if self.privacy.global_privacy_control
            && !native_defaults_verified
            && self.inject_gpc_dnt_headers
        {
            limitations.push(PrivacyLimitation::GlobalPrivacyControlIsBestEffort);
        } else if self.privacy.global_privacy_control && !native_defaults_verified {
            limitations.push(PrivacyLimitation::GlobalPrivacyControlNotVerified);
        }

        PrivacyStatus {
            policy: self.privacy.clone(),
            native_defaults: if native_defaults_verified {
                PrivacyControlStatus::NativeDefault
            } else {
                PrivacyControlStatus::Unavailable
            },
            identity_isolation: if identity_isolated {
                PrivacyControlStatus::BrowserContext
            } else if self.privacy.require_isolated_identity {
                PrivacyControlStatus::Unavailable
            } else {
                PrivacyControlStatus::Disabled
            },
            fingerprint_protection: if self.randomize_fingerprints {
                PrivacyControlStatus::BestEffort
            } else {
                PrivacyControlStatus::Disabled
            },
            // Generic CDP does not expose reliable readback for these controls.
            // Report intent in `policy`, but never convert launcher arguments or
            // configuration flags into enforcement claims.
            secure_dns: PrivacyControlStatus::Unavailable,
            global_privacy_control: if native_defaults_verified
                && self.privacy.global_privacy_control
            {
                PrivacyControlStatus::NativeDefault
            } else if self.privacy.global_privacy_control && self.inject_gpc_dnt_headers {
                PrivacyControlStatus::BestEffort
            } else if self.privacy.global_privacy_control {
                PrivacyControlStatus::Unavailable
            } else {
                PrivacyControlStatus::Disabled
            },
            privacy_sandbox_apis: if native_defaults_verified {
                PrivacyControlStatus::NativeDefault
            } else {
                PrivacyControlStatus::Unavailable
            },
            webrtc_ip_policy: PrivacyControlStatus::Unavailable,
            jit: PrivacyControlStatus::Unavailable,
            dangerous_apis: PrivacyControlStatus::Unavailable,
            downloads: PrivacyControlStatus::Unavailable,
            webgl: PrivacyControlStatus::Unavailable,
            webgpu: PrivacyControlStatus::Unavailable,
            media: PrivacyControlStatus::Unavailable,
            request_filter_bundle: PrivacyControlStatus::Unavailable,
            security_service: PrivacyControlStatus::Unavailable,
            component_updates: PrivacyControlStatus::Unavailable,
            limitations,
        }
    }
}

/// Central Profile Manager for Thor tabs and engine contexts.
///
/// Internally Arc-backed so a single process-wide instance (via `AppState::global().profiles()`)
/// can be shared across IPC handlers. Prefer the singleton over `ProfileManager::new()` per call.
#[derive(Debug, Clone)]
pub struct ProfileManager {
    active_profile: Arc<RwLock<ProfileConfig>>,
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileManager {
    pub fn new() -> Self {
        Self {
            active_profile: Arc::new(RwLock::new(ProfileConfig::stealth_target_default())),
        }
    }

    pub fn get_profile(&self) -> ProfileConfig {
        self.active_profile
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_mode(&self, mode: ProfileMode) {
        let config = match mode {
            ProfileMode::DevHost => ProfileConfig::dev_host_default(),
            ProfileMode::StealthTarget => ProfileConfig::stealth_target_default(),
            ProfileMode::HardenedTarget => ProfileConfig::hardened_target_default(),
        };
        if let Ok(mut lock) = self.active_profile.write() {
            *lock = config;
        }
    }

    pub fn toggle_mode(&self) -> ProfileMode {
        let current_mode = self.get_profile().mode;
        let next_mode = match current_mode {
            ProfileMode::StealthTarget => ProfileMode::HardenedTarget,
            ProfileMode::HardenedTarget => ProfileMode::DevHost,
            ProfileMode::DevHost => ProfileMode::StealthTarget,
        };
        self.set_mode(next_mode);
        next_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_manager_defaults_and_toggle() {
        let manager = ProfileManager::new();
        assert_eq!(manager.get_profile().mode, ProfileMode::StealthTarget);
        assert!(!manager.get_profile().enable_ide_bridge);
        assert!(manager.get_profile().randomize_fingerprints);
        assert_eq!(
            manager.get_profile().privacy.webrtc_ip_policy,
            WebRtcExposurePolicy::DisableNonProxiedUdp
        );

        let toggled = manager.toggle_mode();
        assert_eq!(toggled, ProfileMode::HardenedTarget);
        assert!(!manager.get_profile().allow_webgl);
        assert!(manager.get_profile().privacy.require_isolated_identity);
        assert_eq!(
            manager.get_profile().privacy.webrtc_ip_policy,
            WebRtcExposurePolicy::DisableNonProxiedUdp
        );

        let toggled = manager.toggle_mode();
        assert_eq!(toggled, ProfileMode::DevHost);
        assert!(manager.get_profile().enable_ide_bridge);
        assert!(!manager.get_profile().randomize_fingerprints);

        let toggled = manager.toggle_mode();
        assert_eq!(toggled, ProfileMode::StealthTarget);
    }

    #[test]
    fn profile_modes_have_stable_owned_chromium_values() {
        assert_eq!(ProfileMode::DevHost.chromium_value(), "devhost");
        assert_eq!(ProfileMode::StealthTarget.chromium_value(), "stealth");
        assert_eq!(ProfileMode::HardenedTarget.chromium_value(), "hardened");
    }

    #[test]
    fn privacy_status_does_not_overclaim_unverified_controls() {
        let config = ProfileConfig::hardened_target_default();
        let status = config.privacy_status(false, false);

        assert_eq!(status.native_defaults, PrivacyControlStatus::Unavailable);
        assert_eq!(status.identity_isolation, PrivacyControlStatus::Unavailable);
        assert_eq!(
            status.fingerprint_protection,
            PrivacyControlStatus::BestEffort
        );
        assert_eq!(
            status.global_privacy_control,
            PrivacyControlStatus::BestEffort
        );
        assert_eq!(status.secure_dns, PrivacyControlStatus::Unavailable);
        assert_eq!(
            status.privacy_sandbox_apis,
            PrivacyControlStatus::Unavailable
        );
        assert_eq!(status.jit, PrivacyControlStatus::Unavailable);
        assert_eq!(status.dangerous_apis, PrivacyControlStatus::Unavailable);
        assert_eq!(status.downloads, PrivacyControlStatus::Unavailable);
        assert_eq!(status.webgl, PrivacyControlStatus::Unavailable);
        assert_eq!(status.webgpu, PrivacyControlStatus::Unavailable);
        assert_eq!(status.media, PrivacyControlStatus::Unavailable);
        assert_eq!(
            status.request_filter_bundle,
            PrivacyControlStatus::Unavailable
        );
        assert_eq!(status.security_service, PrivacyControlStatus::Unavailable);
        assert_eq!(status.component_updates, PrivacyControlStatus::Unavailable);
        assert!(status
            .limitations
            .contains(&PrivacyLimitation::IdentityContextRequired));
        assert!(status
            .limitations
            .contains(&PrivacyLimitation::PrivacySandboxApisNotVerified));
    }

    #[test]
    fn privacy_status_reports_real_browser_context() {
        let config = ProfileConfig::hardened_target_default();
        let status = config.privacy_status(true, true);

        assert_eq!(status.native_defaults, PrivacyControlStatus::NativeDefault);
        assert_eq!(
            status.privacy_sandbox_apis,
            PrivacyControlStatus::NativeDefault
        );
        assert_eq!(
            status.global_privacy_control,
            PrivacyControlStatus::NativeDefault
        );
        assert_eq!(
            status.identity_isolation,
            PrivacyControlStatus::BrowserContext
        );
        assert!(!status
            .limitations
            .contains(&PrivacyLimitation::IdentityContextRequired));
        assert!(!status
            .limitations
            .contains(&PrivacyLimitation::PrivacySandboxApisNotVerified));
    }

    #[test]
    fn common_gpc_default_does_not_overclaim_unverified_dev_host() {
        let config = ProfileConfig::dev_host_default();
        assert!(config.privacy.global_privacy_control);

        let unverified = config.privacy_status(false, false);
        assert_eq!(
            unverified.global_privacy_control,
            PrivacyControlStatus::Unavailable
        );
        assert!(unverified
            .limitations
            .contains(&PrivacyLimitation::GlobalPrivacyControlNotVerified));
        assert!(!unverified
            .limitations
            .contains(&PrivacyLimitation::GlobalPrivacyControlIsBestEffort));

        let verified = config.privacy_status(true, false);
        assert_eq!(
            verified.global_privacy_control,
            PrivacyControlStatus::NativeDefault
        );
    }
}
