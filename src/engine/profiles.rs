use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Browser Profile Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileMode {
    /// Host / Dev Profile: Full WebGL, un-sandboxed dev APIs, IDE bridge enabled, no fingerprint noise
    DevHost,
    /// Stealth Target Profile: Hardened anti-tracking, randomized Canvas/WebGL noise, strict GPC/DNT headers
    StealthTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub mode: ProfileMode,
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
            strip_tracking_params: true,
            inject_gpc_dnt_headers: true,
            randomize_fingerprints: true,
            allow_webgl: true, // Controlled canvas noise
            enable_ide_bridge: false,
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
            active_profile: Arc::new(RwLock::new(ProfileConfig::dev_host_default())),
        }
    }

    pub fn get_profile(&self) -> ProfileConfig {
        self.active_profile.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_mode(&self, mode: ProfileMode) {
        let config = match mode {
            ProfileMode::DevHost => ProfileConfig::dev_host_default(),
            ProfileMode::StealthTarget => ProfileConfig::stealth_target_default(),
        };
        if let Ok(mut lock) = self.active_profile.write() {
            *lock = config;
        }
    }

    pub fn toggle_mode(&self) -> ProfileMode {
        let current_mode = self.get_profile().mode;
        let next_mode = match current_mode {
            ProfileMode::DevHost => ProfileMode::StealthTarget,
            ProfileMode::StealthTarget => ProfileMode::DevHost,
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
        assert_eq!(manager.get_profile().mode, ProfileMode::DevHost);
        assert!(manager.get_profile().enable_ide_bridge);
        assert!(!manager.get_profile().randomize_fingerprints);

        let toggled = manager.toggle_mode();
        assert_eq!(toggled, ProfileMode::StealthTarget);
        assert!(!manager.get_profile().enable_ide_bridge);
        assert!(manager.get_profile().randomize_fingerprints);
        assert!(manager.get_profile().strip_tracking_params);
    }
}
