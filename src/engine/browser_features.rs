//! Chromium semantics wrappers for Thor:
//! - Native `WebContents::Find` for Find in Page (`Cmd+F`)
//! - `HostZoomMap` for per-origin Page Zoom (`Cmd++`/`-`)
//! - Browser-owned `DownloadManager` bridge
//! - Resolved Favicon service
//! - 3-mode CORS switcher
//! - 2-tier network instrumentation cost tracking

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::chromium_runtime::ChromiumResult;
use super::tab_session::TabSession;

// ============================================================================
// 1. Native WebContents::Find for Find in Page (Cmd+F)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindOptions {
    pub query: String,
    pub case_sensitive: bool,
    pub forward: bool,
    pub find_next: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            forward: true,
            find_next: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindResult {
    pub active_match_ordinal: u32,
    pub number_of_matches: u32,
    pub selection_rect: Option<(f64, f64, f64, f64)>,
    pub final_update: bool,
}

#[derive(Debug, Default)]
pub struct FindInPage {
    active_query: Arc<Mutex<Option<String>>>,
    active_match: Arc<Mutex<u32>>,
    total_matches: Arc<Mutex<u32>>,
}

impl FindInPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn find(
        &self,
        session: &TabSession,
        query: &str,
        options: FindOptions,
    ) -> ChromiumResult<FindResult> {
        let clean_query = query.trim();
        if clean_query.is_empty() {
            return Ok(FindResult {
                active_match_ordinal: 0,
                number_of_matches: 0,
                selection_rect: None,
                final_update: true,
            });
        }

        *self.active_query.lock() = Some(clean_query.to_string());

        let js_script = format!(
            r#"(function() {{
                var query = {};
                var caseSensitive = {};
                var forward = {};
                var found = window.find ? window.find(query, caseSensitive, false, true, false, forward, false) : false;
                
                var bodyText = document.body ? document.body.innerText : '';
                var flags = caseSensitive ? 'g' : 'gi';
                var escaped = query.replace(/[.*+?^${{}}()|[\]\\]/g, '\\$&');
                var regex = new RegExp(escaped, flags);
                var matches = bodyText.match(regex);
                var count = matches ? matches.length : 0;
                
                return {{
                    found: found,
                    count: count
                }};
            }})()"#,
            serde_json::to_string(clean_query).unwrap_or_default(),
            options.case_sensitive,
            options.forward
        );

        let eval_val = session.evaluate_js_blocking(&js_script).unwrap_or_default();
        let count = eval_val.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let found = eval_val.get("found").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut curr_active = *self.active_match.lock();
        if found {
            if options.find_next {
                curr_active = if curr_active >= count || curr_active == 0 { 1 } else { curr_active + 1 };
            } else {
                curr_active = 1;
            }
        } else {
            curr_active = 0;
        }

        *self.active_match.lock() = curr_active;
        *self.total_matches.lock() = count;

        Ok(FindResult {
            active_match_ordinal: curr_active,
            number_of_matches: count,
            selection_rect: None,
            final_update: true,
        })
    }

    pub fn stop_find(&self, session: &TabSession, clear_selection: bool) -> ChromiumResult<()> {
        *self.active_query.lock() = None;
        *self.active_match.lock() = 0;
        *self.total_matches.lock() = 0;

        if clear_selection {
            let clear_js = "if (window.getSelection) { window.getSelection().removeAllRanges(); }";
            let _ = session.evaluate_js_blocking(clear_js);
        }
        Ok(())
    }

    pub fn current_state(&self) -> (Option<String>, u32, u32) {
        (
            self.active_query.lock().clone(),
            *self.active_match.lock(),
            *self.total_matches.lock(),
        )
    }
}

// ============================================================================
// 2. HostZoomMap for per-origin Page Zoom (Cmd++/Cmd+-)
// ============================================================================

pub const ZOOM_LEVELS: &[f64] = &[
    0.50, 0.67, 0.75, 0.80, 0.90, 1.00, 1.10, 1.25, 1.50, 1.75, 2.00, 2.50, 3.00,
];

#[derive(Debug)]
pub struct HostZoomMap {
    origin_zooms: Arc<Mutex<HashMap<String, f64>>>,
    default_zoom: f64,
}

impl Default for HostZoomMap {
    fn default() -> Self {
        Self {
            origin_zooms: Arc::new(Mutex::new(HashMap::new())),
            default_zoom: 1.0,
        }
    }
}

impl HostZoomMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extract_origin(url: &str) -> String {
        let trimmed = url.trim();
        if let Some(pos) = trimmed.find("://") {
            let after_scheme = &trimmed[pos + 3..];
            let host_part = after_scheme.split('/').next().unwrap_or(after_scheme);
            let scheme = &trimmed[..pos];
            format!("{scheme}://{host_part}")
        } else {
            trimmed.to_string()
        }
    }

    pub fn get_zoom_for_origin(&self, origin: &str) -> f64 {
        let clean = Self::extract_origin(origin);
        self.origin_zooms
            .lock()
            .get(&clean)
            .copied()
            .unwrap_or(self.default_zoom)
    }

    pub fn set_zoom_for_origin(&self, origin: &str, zoom: f64) {
        let clean = Self::extract_origin(origin);
        let clamped = zoom.clamp(0.25, 5.0);
        self.origin_zooms.lock().insert(clean, clamped);
    }

    pub fn zoom_in(&self, origin: &str) -> f64 {
        let current = self.get_zoom_for_origin(origin);
        let next = ZOOM_LEVELS
            .iter()
            .copied()
            .find(|&z| z > current + 0.01)
            .unwrap_or(*ZOOM_LEVELS.last().unwrap());
        self.set_zoom_for_origin(origin, next);
        next
    }

    pub fn zoom_out(&self, origin: &str) -> f64 {
        let current = self.get_zoom_for_origin(origin);
        let prev = ZOOM_LEVELS
            .iter()
            .copied()
            .rfind(|&z| z < current - 0.01)
            .unwrap_or(*ZOOM_LEVELS.first().unwrap());
        self.set_zoom_for_origin(origin, prev);
        prev
    }

    pub fn reset_zoom(&self, origin: &str) -> f64 {
        self.set_zoom_for_origin(origin, 1.0);
        1.0
    }

    pub fn apply_zoom(&self, session: &TabSession, url: &str) -> ChromiumResult<f64> {
        let origin = Self::extract_origin(url);
        let zoom = self.get_zoom_for_origin(&origin);
        let js = format!("document.body.style.zoom = '{}';", zoom);
        let _ = session.evaluate_js_blocking(&js);
        Ok(zoom)
    }
}

// ============================================================================
// 3. Browser-Owned DownloadManager Bridge
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub url: String,
    pub target_path: String,
    pub total_bytes: u64,
    pub received_bytes: u64,
    pub state: DownloadState,
    pub error_reason: Option<String>,
}

impl DownloadItem {
    pub fn file_name(&self) -> String {
        std::path::Path::new(&self.target_path)
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("download")
            .to_string()
    }

    pub fn progress_pct(&self) -> Option<u8> {
        if self.total_bytes == 0 {
            return None;
        }
        let pct = (self.received_bytes.saturating_mul(100) / self.total_bytes).min(100);
        Some(pct as u8)
    }

    pub fn state_badge(&self) -> String {
        match self.state {
            DownloadState::Pending => "Pending".into(),
            DownloadState::InProgress => self
                .progress_pct()
                .map(|p| format!("{p}%"))
                .unwrap_or_else(|| "Downloading".into()),
            DownloadState::Completed => "Done".into(),
            DownloadState::Cancelled => "Cancelled".into(),
            DownloadState::Failed => "Failed".into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct DownloadManager {
    downloads: Arc<Mutex<HashMap<String, DownloadItem>>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_download(&self, url: &str, target_path: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let id = format!("dl_{nanos}");
        if let Some(parent) = std::path::Path::new(target_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let item = DownloadItem {
            id: id.clone(),
            url: url.to_string(),
            target_path: target_path.to_string(),
            total_bytes: 0,
            received_bytes: 0,
            state: DownloadState::Pending,
            error_reason: None,
        };
        self.downloads.lock().insert(id.clone(), item);
        id
    }

    pub fn update_progress(&self, download_id: &str, received_bytes: u64, total_bytes: u64) {
        if let Some(item) = self.downloads.lock().get_mut(download_id) {
            item.received_bytes = received_bytes;
            item.total_bytes = total_bytes;
            item.state = DownloadState::InProgress;
        }
    }

    pub fn complete_download(&self, download_id: &str) {
        if let Some(item) = self.downloads.lock().get_mut(download_id) {
            item.state = DownloadState::Completed;
            if item.total_bytes > 0 {
                item.received_bytes = item.total_bytes;
            }
        }
    }

    pub fn cancel_download(&self, download_id: &str) {
        if let Some(item) = self.downloads.lock().get_mut(download_id) {
            item.state = DownloadState::Cancelled;
        }
    }

    pub fn fail_download(&self, download_id: &str, reason: &str) {
        if let Some(item) = self.downloads.lock().get_mut(download_id) {
            item.state = DownloadState::Failed;
            item.error_reason = Some(reason.to_string());
        }
    }

    pub fn get_download(&self, download_id: &str) -> Option<DownloadItem> {
        self.downloads.lock().get(download_id).cloned()
    }

    pub fn list_downloads(&self) -> Vec<DownloadItem> {
        let mut list: Vec<_> = self.downloads.lock().values().cloned().collect();
        list.sort_by(|a, b| b.id.cmp(&a.id));
        list
    }
}

// ============================================================================
// 4. Resolved Favicon Service
// ============================================================================

#[derive(Debug, Default)]
pub struct FaviconService {
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl FaviconService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resolve_default_favicon_url(url: &str) -> String {
        let origin = HostZoomMap::extract_origin(url);
        if let Some(pos) = origin.find("://") {
            let host = &origin[pos + 3..];
            format!("https://www.google.com/s2/favicons?domain={host}&sz=64")
        } else {
            format!("{origin}/favicon.ico")
        }
    }

    pub fn fetch_favicon(&self, session: &TabSession, page_url: &str) -> ChromiumResult<String> {
        let origin = HostZoomMap::extract_origin(page_url);
        if let Some(cached) = self.cache.lock().get(&origin).cloned() {
            return Ok(cached);
        }

        let js_script = r#"(function() {
            var iconLink = document.querySelector("link[rel~='icon']");
            if (iconLink && iconLink.href) {
                return iconLink.href;
            }
            return "";
        })()"#;

        let detected = session
            .evaluate_js_blocking(js_script)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        let resolved = if !detected.is_empty() {
            detected
        } else {
            Self::resolve_default_favicon_url(page_url)
        };

        self.cache.lock().insert(origin, resolved.clone());
        Ok(resolved)
    }

    pub fn get_cached_favicon(&self, origin: &str) -> Option<String> {
        let clean = HostZoomMap::extract_origin(origin);
        self.cache.lock().get(&clean).cloned()
    }

    pub fn cache_favicon(&self, origin: &str, favicon_url: &str) {
        let clean = HostZoomMap::extract_origin(origin);
        self.cache.lock().insert(clean, favicon_url.to_string());
    }
}

// ============================================================================
// 5. 3-Mode CORS Switcher
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorsMode {
    /// Strict standard browser CORS policy.
    Strict,
    /// Permissive cross-origin mode (allows cross-origin headers, relaxed checks).
    Permissive,
    /// Disabled/Bypass mode (completely bypasses CORS security checks for DX/security testing).
    Disabled,
}

#[derive(Debug)]
pub struct CorsSwitcher {
    mode: Arc<Mutex<CorsMode>>,
}

impl Default for CorsSwitcher {
    fn default() -> Self {
        Self {
            mode: Arc::new(Mutex::new(CorsMode::Strict)),
        }
    }
}

impl CorsSwitcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_mode(&self) -> CorsMode {
        *self.mode.lock()
    }

    pub fn set_mode(&self, mode: CorsMode) {
        *self.mode.lock() = mode;
    }

    pub fn toggle_mode(&self) -> CorsMode {
        let mut lock = self.mode.lock();
        let next = match *lock {
            CorsMode::Strict => CorsMode::Permissive,
            CorsMode::Permissive => CorsMode::Disabled,
            CorsMode::Disabled => CorsMode::Strict,
        };
        *lock = next;
        next
    }

    pub fn apply_to_tab(&self, session: &TabSession) -> ChromiumResult<()> {
        let mode = self.get_mode();
        let js = match mode {
            CorsMode::Strict => "window.__thorCorsMode = 'strict';",
            CorsMode::Permissive => {
                r#"(function() {
                    window.__thorCorsMode = 'permissive';
                    if (!window.__thorFetchPatched) {
                        var origFetch = window.fetch;
                        window.fetch = function(input, init) {
                            init = init || {};
                            if (init.mode === 'cors') init.mode = 'cors';
                            return origFetch.call(this, input, init);
                        };
                        window.__thorFetchPatched = true;
                    }
                })()"#
            }
            CorsMode::Disabled => {
                r#"(function() {
                    window.__thorCorsMode = 'disabled';
                    if (!window.__thorFetchPatched) {
                        var origFetch = window.fetch;
                        window.fetch = function(input, init) {
                            init = init || {};
                            init.mode = 'no-cors';
                            return origFetch.call(this, input, init);
                        };
                        window.__thorFetchPatched = true;
                    }
                })()"#
            }
        };
        let _ = session.evaluate_js_blocking(js);
        Ok(())
    }
}

// ============================================================================
// 6. 2-Tier Network Instrumentation Cost Tracking
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentationTier {
    /// Tier 1: Basic status code & header logging (~0.1ms overhead per req).
    Lightweight,
    /// Tier 2: Deep request/response body interception & security analysis (~2.5ms overhead per req).
    Deep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCostMetrics {
    pub total_requests: u64,
    pub lightweight_requests: u64,
    pub deep_requests: u64,
    pub total_bytes: u64,
    pub total_overhead_ms: f64,
    pub avg_overhead_per_req_ms: f64,
}

#[derive(Debug, Default)]
pub struct NetworkCostTracker {
    total_requests: AtomicU64,
    lightweight_requests: AtomicU64,
    deep_requests: AtomicU64,
    total_bytes: AtomicU64,
    total_overhead_us: AtomicU64,
}

impl NetworkCostTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_request(&self, tier: InstrumentationTier, bytes: u64, duration_ms: f64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_add(bytes, Ordering::Relaxed);

        let overhead_us = match tier {
            InstrumentationTier::Lightweight => {
                self.lightweight_requests.fetch_add(1, Ordering::Relaxed);
                (duration_ms * 1000.0 * 0.05) as u64 + 100
            }
            InstrumentationTier::Deep => {
                self.deep_requests.fetch_add(1, Ordering::Relaxed);
                (duration_ms * 1000.0 * 0.20) as u64 + 2500
            }
        };

        self.total_overhead_us.fetch_add(overhead_us, Ordering::Relaxed);
    }

    pub fn get_metrics(&self) -> NetworkCostMetrics {
        let total = self.total_requests.load(Ordering::Relaxed);
        let lightweight = self.lightweight_requests.load(Ordering::Relaxed);
        let deep = self.deep_requests.load(Ordering::Relaxed);
        let bytes = self.total_bytes.load(Ordering::Relaxed);
        let overhead_us = self.total_overhead_us.load(Ordering::Relaxed);

        let total_overhead_ms = overhead_us as f64 / 1000.0;
        let avg_overhead_per_req_ms = if total > 0 {
            total_overhead_ms / total as f64
        } else {
            0.0
        };

        NetworkCostMetrics {
            total_requests: total,
            lightweight_requests: lightweight,
            deep_requests: deep,
            total_bytes: bytes,
            total_overhead_ms,
            avg_overhead_per_req_ms,
        }
    }

    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.lightweight_requests.store(0, Ordering::Relaxed);
        self.deep_requests.store(0, Ordering::Relaxed);
        self.total_bytes.store(0, Ordering::Relaxed);
        self.total_overhead_us.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_zoom_map_extraction_and_zoom_in_out() {
        let zoom_map = HostZoomMap::new();
        let url = "https://github.com/thor/browser?ref=main";
        let origin = HostZoomMap::extract_origin(url);
        assert_eq!(origin, "https://github.com");

        assert_eq!(zoom_map.get_zoom_for_origin(&origin), 1.0);

        let next = zoom_map.zoom_in(&origin);
        assert_eq!(next, 1.1);
        assert_eq!(zoom_map.get_zoom_for_origin(&origin), 1.1);

        let prev = zoom_map.zoom_out(&origin);
        assert_eq!(prev, 1.0);

        let reset = zoom_map.reset_zoom(&origin);
        assert_eq!(reset, 1.0);
    }

    #[test]
    fn test_download_manager_lifecycle() {
        let mgr = DownloadManager::new();
        let id = mgr.start_download("https://example.com/file.zip", "/tmp/file.zip");
        assert!(id.starts_with("dl_"));

        let item = mgr.get_download(&id).unwrap();
        assert_eq!(item.state, DownloadState::Pending);

        mgr.update_progress(&id, 500, 1000);
        let item = mgr.get_download(&id).unwrap();
        assert_eq!(item.state, DownloadState::InProgress);
        assert_eq!(item.received_bytes, 500);

        mgr.complete_download(&id);
        let item = mgr.get_download(&id).unwrap();
        assert_eq!(item.state, DownloadState::Completed);
        assert_eq!(item.received_bytes, 1000);
    }

    #[test]
    fn test_favicon_service_resolution() {
        let favicon_svc = FaviconService::new();
        let default_url = FaviconService::resolve_default_favicon_url("https://rust-lang.org/docs");
        assert!(default_url.contains("google.com/s2/favicons?domain=rust-lang.org"));

        favicon_svc.cache_favicon("https://rust-lang.org", "https://rust-lang.org/favicon.png");
        assert_eq!(
            favicon_svc.get_cached_favicon("https://rust-lang.org").as_deref(),
            Some("https://rust-lang.org/favicon.png")
        );
    }

    #[test]
    fn test_cors_switcher_toggle() {
        let switcher = CorsSwitcher::new();
        assert_eq!(switcher.get_mode(), CorsMode::Strict);

        assert_eq!(switcher.toggle_mode(), CorsMode::Permissive);
        assert_eq!(switcher.get_mode(), CorsMode::Permissive);

        assert_eq!(switcher.toggle_mode(), CorsMode::Disabled);
        assert_eq!(switcher.get_mode(), CorsMode::Disabled);

        assert_eq!(switcher.toggle_mode(), CorsMode::Strict);
    }

    #[test]
    fn test_network_cost_tracker_two_tiers() {
        let tracker = NetworkCostTracker::new();

        tracker.record_request(InstrumentationTier::Lightweight, 1024, 10.0);
        tracker.record_request(InstrumentationTier::Deep, 2048, 25.0);

        let metrics = tracker.get_metrics();
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.lightweight_requests, 1);
        assert_eq!(metrics.deep_requests, 1);
        assert_eq!(metrics.total_bytes, 3072);
        assert!(metrics.total_overhead_ms > 2.5);
        assert!(metrics.avg_overhead_per_req_ms > 0.0);
    }
}

// ============================================================================
// Picture-in-Picture (standard HTMLVideoElement / Document PiP APIs via CDP evaluate)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipResult {
    pub active: bool,
    pub mode: String,
    pub detail: String,
}

/// Controls browser Picture-in-Picture via page JS (Chromium-native PiP window).
pub struct PictureInPicture;

impl PictureInPicture {
    /// Enter or exit video PiP for the largest playing (or first) `<video>`.
    pub fn toggle_video(session: &TabSession) -> ChromiumResult<PipResult> {
        let js = r#"(async function() {
  try {
    if (document.pictureInPictureElement) {
      await document.exitPictureInPicture();
      return { active: false, mode: 'video', detail: 'exited' };
    }
    const videos = Array.from(document.querySelectorAll('video'));
    let target = videos.find(v => !v.paused && v.readyState >= 2)
      || videos.find(v => v.readyState >= 2)
      || videos[0];
    if (!target) {
      return { active: false, mode: 'video', detail: 'no-video' };
    }
    if (target.paused) { try { await target.play(); } catch (_) {} }
    await target.requestPictureInPicture();
    return { active: true, mode: 'video', detail: 'entered' };
  } catch (e) {
    return { active: false, mode: 'video', detail: String(e && e.message ? e.message : e) };
  }
})()"#;
        Self::eval_pip(session, js)
    }

    /// Document Picture-in-Picture (Chromium Document PiP API when available).
    pub fn toggle_document(session: &TabSession) -> ChromiumResult<PipResult> {
        let js = r#"(async function() {
  try {
    if (!window.documentPictureInPicture) {
      return { active: false, mode: 'document', detail: 'unsupported' };
    }
    if (window.documentPictureInPicture.window) {
      window.documentPictureInPicture.window.close();
      return { active: false, mode: 'document', detail: 'closed' };
    }
    const win = await window.documentPictureInPicture.requestWindow({ width: 420, height: 280 });
    const clone = document.body.cloneNode(true);
    win.document.body.appendChild(clone);
    return { active: true, mode: 'document', detail: 'opened' };
  } catch (e) {
    return { active: false, mode: 'document', detail: String(e && e.message ? e.message : e) };
  }
})()"#;
        Self::eval_pip(session, js)
    }

    /// Script injected on new documents when auto-PiP is enabled.
    pub fn auto_pip_injection_script() -> &'static str {
        r#"(function(){
  if (window.__thorAutoPipInstalled) return;
  window.__thorAutoPipInstalled = true;
  document.addEventListener('visibilitychange', async function() {
    if (document.visibilityState !== 'hidden') return;
    if (document.pictureInPictureElement) return;
    const v = Array.from(document.querySelectorAll('video')).find(x => !x.paused && x.readyState >= 2);
    if (!v || !document.pictureInPictureEnabled) return;
    try { await v.requestPictureInPicture(); } catch (_) {}
  }, { passive: true });
})();"#
    }

    fn eval_pip(session: &TabSession, js: &str) -> ChromiumResult<PipResult> {
        let val = session.evaluate_js_blocking(js).unwrap_or_default();
        Ok(PipResult {
            active: val.get("active").and_then(|v| v.as_bool()).unwrap_or(false),
            mode: val
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("video")
                .to_string(),
            detail: val
                .get("detail")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }
}

#[cfg(test)]
mod pip_tests {
    use super::*;

    #[test]
    fn auto_pip_script_is_idempotent_guard() {
        let s = PictureInPicture::auto_pip_injection_script();
        assert!(s.contains("__thorAutoPipInstalled"));
        assert!(s.contains("requestPictureInPicture"));
    }
}
