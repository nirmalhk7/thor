use crate::dx::bitwarden::BitwardenVault;
use crate::engine::browser_features::DownloadManager;
use crate::engine::engine_coordinator::EngineHandle;
use crate::engine::profiles::{ProfileConfig, ProfileManager, ProfileMode};
use crate::features::FeatureEngine;
use crate::net::consent::{validate_locust_start, LocustStartGate};
use crate::net::locust::{LocustHandle, LocustScoredMetrics};
use crate::net::security_shield::SecurityShield;
use crate::net::tamper::TrafficTamperEngine;
use crate::paths::thor_data_dir;
use parking_lot::Mutex;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Process-wide toggles for SecurityShield behavior on navigate / request injection.
#[derive(Debug)]
pub struct SecurityShieldToggles {
    strip_tracking: AtomicBool,
    inject_headers: AtomicBool,
}

impl Default for SecurityShieldToggles {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityShieldToggles {
    /// Defaults match DevHost (SEC-010): strip/inject off until profile sync.
    pub fn new() -> Self {
        Self {
            strip_tracking: AtomicBool::new(false),
            inject_headers: AtomicBool::new(false),
        }
    }

    pub fn set_strip_tracking(&self, enabled: bool) {
        self.strip_tracking.store(enabled, Ordering::SeqCst);
    }

    pub fn strip_tracking_enabled(&self) -> bool {
        self.strip_tracking.load(Ordering::SeqCst)
    }

    pub fn set_inject_headers(&self, enabled: bool) {
        self.inject_headers.store(enabled, Ordering::SeqCst);
    }

    pub fn inject_headers_enabled(&self) -> bool {
        self.inject_headers.load(Ordering::SeqCst)
    }

    pub fn apply_profile(&self, cfg: &ProfileConfig) {
        self.set_strip_tracking(cfg.strip_tracking_params);
        self.set_inject_headers(cfg.inject_gpc_dnt_headers);
    }

    /// Sanitize a navigation URL when strip-tracking is enabled; otherwise pass through.
    pub fn sanitize_navigate_url(&self, url: &str) -> (String, bool) {
        if self.strip_tracking_enabled() {
            SecurityShield::sanitize_url(url)
        } else {
            (url.to_string(), false)
        }
    }

    /// Security headers to inject when the inject-headers toggle is on.
    pub fn headers_for_request(&self) -> Vec<(&'static str, &'static str)> {
        if self.inject_headers_enabled() {
            SecurityShield::get_security_headers()
        } else {
            Vec::new()
        }
    }
}

fn profile_store_path() -> std::path::PathBuf {
    thor_data_dir().join("profile.json")
}

fn load_persisted_mode() -> Option<ProfileMode> {
    let path = profile_store_path();
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<ProfileMode>(&raw).ok()
}

fn persist_mode(mode: ProfileMode) {
    let dir = thor_data_dir();
    let _ = fs::create_dir_all(&dir);
    let path = profile_store_path();
    if let Ok(raw) = serde_json::to_string(&mode) {
        let _ = fs::write(path, raw);
    }
}

/// Process-wide singleton holding net/security/DX engines shared across IPC handlers.
///
/// Access exclusively via [`AppState::global`] — do not construct per-IPC
/// `TrafficTamperEngine::new()` / `ProfileManager::new()` instances.
pub struct AppState {
    profiles: ProfileManager,
    tamper: TrafficTamperEngine,
    idor: crate::net::idor_matrix::IdorMatrixEngine,
    security: SecurityShieldToggles,
    tab_history: Mutex<crate::dx::cmd_palette::TabHistoryTracker>,
    locust: Mutex<Option<LocustHandle>>,
    features: FeatureEngine,
    engine_handle: Mutex<Option<EngineHandle>>,
    vault: BitwardenVault,
    downloads: DownloadManager,
    /// Auto Picture-in-Picture when tab hides while video plays.
    auto_pip: AtomicBool,
    /// Run passive Visit Recon after each navigation.
    visit_recon_enabled: AtomicBool,
    last_visit_recon: Mutex<Option<crate::dx::visit_recon::VisitReconReport>>,
    projects: OnceLock<Result<crate::projects::ProjectStore, String>>,
}

static APP_STATE: OnceLock<AppState> = OnceLock::new();

impl AppState {
    fn new() -> Self {
        let profiles = ProfileManager::new();
        if let Some(mode) = load_persisted_mode() {
            profiles.set_mode(mode);
        }
        let security = SecurityShieldToggles::new();
        security.apply_profile(&profiles.get_profile());
        // Interception off by default (SEC-020).
        let tamper = TrafficTamperEngine::new();
        tamper.set_interception_enabled(false);
        let idor = crate::net::idor_matrix::IdorMatrixEngine::new();

        Self {
            profiles,
            tamper,
            idor,
            security,
            tab_history: Mutex::new(crate::dx::cmd_palette::TabHistoryTracker::new()),
            locust: Mutex::new(None),
            features: FeatureEngine::new(),
            engine_handle: Mutex::new(None),
            vault: BitwardenVault::new(),
            downloads: DownloadManager::new(),
            auto_pip: AtomicBool::new(true),
            visit_recon_enabled: AtomicBool::new(true),
            last_visit_recon: Mutex::new(None),
            projects: OnceLock::new(),
        }
    }

    /// Process-wide singleton. Lazily initialized on first call.
    pub fn global() -> &'static AppState {
        APP_STATE.get_or_init(AppState::new)
    }

    pub fn profiles(&self) -> &ProfileManager {
        &self.profiles
    }

    pub fn projects(&self) -> Result<&crate::projects::ProjectStore, String> {
        self.projects.get_or_init(|| {
            crate::projects::ProjectStore::open(&thor_data_dir().join("Projects/projects.sqlite3"))
                .map_err(|error| error.to_string())
        }).as_ref().map_err(Clone::clone)
    }

    pub fn tamper(&self) -> &TrafficTamperEngine {
        &self.tamper
    }

    pub fn idor(&self) -> &crate::net::idor_matrix::IdorMatrixEngine {
        &self.idor
    }

    pub fn security(&self) -> &SecurityShieldToggles {
        &self.security
    }

    pub fn tab_history(&self) -> parking_lot::MutexGuard<'_, crate::dx::cmd_palette::TabHistoryTracker> {
        self.tab_history.lock()
    }

    /// Process-wide Engineer Feature Suite (144 features across six domains).
    pub fn features(&self) -> &FeatureEngine {
        &self.features
    }

    /// Access the in-memory ring buffer for live log queries.
    pub fn logs(&self) -> std::sync::Arc<crate::logging::LogBuffer> {
        crate::logging::Logger::global().buffer()
    }

    /// First-class Bitwarden vault (`bw` CLI; session in memory only).
    pub fn vault(&self) -> &BitwardenVault {
        &self.vault
    }

    /// Browser-owned download ledger (Cmd+⇧K → Downloads).
    pub fn downloads(&self) -> &DownloadManager {
        &self.downloads
    }

    pub fn auto_pip_enabled(&self) -> bool {
        self.auto_pip.load(Ordering::SeqCst)
    }

    pub fn set_auto_pip(&self, enabled: bool) {
        self.auto_pip.store(enabled, Ordering::SeqCst);
    }

    pub fn visit_recon_enabled(&self) -> bool {
        self.visit_recon_enabled.load(Ordering::SeqCst)
    }

    pub fn set_visit_recon_enabled(&self, enabled: bool) {
        self.visit_recon_enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn store_visit_recon(&self, report: crate::dx::visit_recon::VisitReconReport) {
        *self.last_visit_recon.lock() = Some(report);
    }

    pub fn last_visit_recon(&self) -> Option<crate::dx::visit_recon::VisitReconReport> {
        self.last_visit_recon.lock().clone()
    }

    pub fn set_engine_handle(&self, handle: EngineHandle) {
        *self.engine_handle.lock() = Some(handle);
    }

    pub fn engine_handle(&self) -> Option<EngineHandle> {
        self.engine_handle.lock().clone()
    }

    /// Set profile mode, sync shield toggles, persist (FS-PROF-01/02, SEC-010–012).
    pub fn set_profile_mode(&self, mode: ProfileMode) {
        self.profiles.set_mode(mode);
        self.security.apply_profile(&self.profiles.get_profile());
        persist_mode(mode);
    }

    pub fn toggle_profile(&self) -> ProfileMode {
        let next = self.profiles.toggle_mode();
        self.security.apply_profile(&self.profiles.get_profile());
        persist_mode(next);
        next
    }

    /// Gated Locust start (consent + caps). Replaces any prior swarm.
    pub fn locust_start_gated(&self, gate: LocustStartGate) -> Result<(), String> {
        let config = validate_locust_start(gate)?;
        let prev = self.locust.lock().take();
        if let Some(prev) = prev {
            let _ = prev.stop();
        }
        let handle = LocustHandle::start(config)?;
        let dropped = self.locust.lock().replace(handle);
        if let Some(dropped) = dropped {
            let _ = dropped.stop();
        }
        Ok(())
    }

    /// Start (or restart) a Locust HTTP(S) swarm — gated for private/local by default.
    ///
    /// Prefer [`Self::locust_start_gated`] for explicit public_ack.
    pub fn locust_start(
        &self,
        target_url: &str,
        users: u32,
        ramp_up_secs: u32,
        runtime_secs: u64,
    ) -> Result<(), String> {
        self.locust_start_gated(LocustStartGate {
            target_url: target_url.to_string(),
            users,
            ramp_up_secs,
            runtime_secs,
            public_ack: false,
        })
    }

    /// Stop the active Locust swarm and return scored metrics from real HTTP requests.
    pub fn locust_stop(&self) -> Option<LocustScoredMetrics> {
        let prev = self.locust.lock().take();
        prev.map(|h| h.stop())
    }

    /// Snapshot scored metrics without stopping the swarm.
    pub fn locust_snapshot(&self) -> Option<LocustScoredMetrics> {
        self.locust.lock().as_ref().map(|h| h.snapshot())
    }

    pub fn locust_is_running(&self) -> bool {
        self.locust
            .lock()
            .as_ref()
            .map(|h| h.is_running())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn test_app_state_singleton_engines() {
        let a = AppState::global();
        let b = AppState::global();
        assert!(std::ptr::eq(a, b));

        a.set_profile_mode(ProfileMode::StealthTarget);
        assert_eq!(b.profiles().get_profile().mode, ProfileMode::StealthTarget);
        assert!(b.security().strip_tracking_enabled());
        a.set_profile_mode(ProfileMode::DevHost);
        assert!(!b.security().strip_tracking_enabled());

        a.tamper().set_interception_enabled(true);
        assert!(b.tamper().is_interception_enabled());
        a.tamper().set_interception_enabled(false);

        a.security().set_strip_tracking(true);
        let (clean, stripped) = a
            .security()
            .sanitize_navigate_url("https://ex.com/?utm_source=x&ok=1");
        assert!(stripped);
        assert_eq!(clean, "https://ex.com/?ok=1");
        a.security().set_strip_tracking(false);

        a.tab_history()
            .record_visit("tab-1", "https://example.com", "Example");
        assert_eq!(b.tab_history().get_history_for_tab("tab-1").len(), 1);
    }

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_app_state_locust_start_stop() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/");

        let server = std::thread::spawn(move || {
            for _ in 0..32 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 512];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    );
                }
            }
        });

        let state = AppState::global();
        let _ = state.locust_stop();
        state.locust_start(&url, 2, 0, 2).expect("locust_start");
        std::thread::sleep(Duration::from_millis(1000));
        assert!(state.locust_is_running() || state.locust_snapshot().is_some());
        let scored = state.locust_stop().expect("locust_stop metrics");
        assert!(scored.total_requests > 0);
        let _ = server.join();
    }

    #[test]
    fn test_locust_public_blocked_without_ack() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let state = AppState::global();
        let _ = state.locust_stop();
        let err = state.locust_start("https://example.com/", 2, 0, 2);
        assert!(err.is_err());
    }
}
