//! Traffic interceptor & repeater engine (Fetch-ready).
//!
//! # CDP Fetch integration (future wiring)
//!
//! When Chromium Fetch domain interception is enabled (`Fetch.enable` with
//! `requestPattern`), the runtime should call into this engine as follows:
//!
//! 1. On `Fetch.requestPaused` for a request stage, build an [`InterceptedRequest`]
//!    from the CDP event (`requestId`, method, URL, headers, postData).
//! 2. Call [`TrafficTamperEngine::process_outgoing_request`]:
//!    - `Some(req)` → apply header edits and continue via `Fetch.continueRequest`
//!      (or `Fetch.fulfillRequest` if fully mocked).
//!    - `None` → request is parked in the pending queue; wait for UI
//!      [`TrafficTamperEngine::forward_request`] / [`TrafficTamperEngine::drop_request`],
//!      then issue the matching CDP continue / fail call.
//! 3. On response completion, call [`TrafficTamperEngine::record_history`] for the
//!    Repeater / Inspector pane.
//!
//! Prefer the process-wide instance from `AppState::global().tamper()` rather than
//! constructing a new engine per IPC message.

mod rules;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Intercepted HTTP Request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterceptedRequest {
    pub id: String,
    pub timestamp: u64,
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub is_modified: bool,
}

/// Intercepted HTTP Response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterceptedResponse {
    pub request_id: String,
    pub timestamp: u64,
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub duration_ms: u64,
}

/// Rule for automatic matching & modifying traffic on the fly
/// Rule for automatic matching & modifying traffic on the fly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamperRule {
    pub id: String,
    pub enabled: bool,
    pub target_url_pattern: String,
    pub match_header_key: Option<String>,
    pub replace_header_key: String,
    pub replace_header_value: String,
    pub mock_status: Option<u16>,
    pub mock_body: Option<String>,
    pub delay_ms: Option<u64>,
}

/// Result outcome of running a request through TrafficTamperEngine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TamperOutcome {
    /// Pass request through (potentially with modified headers)
    Forward(InterceptedRequest),
    /// Auto-mock response stubbed by TamperRule
    Mock(InterceptedResponse),
    /// Request paused waiting for manual forward/drop in UI
    Pause,
}

/// What the Fetch handler should do after a Pause is resolved by the UI.
#[derive(Debug, Clone)]
pub enum PauseRelease {
    Forward(InterceptedRequest),
    Drop,
}

#[derive(Debug)]
struct TamperEngineInner {
    interception_enabled: AtomicBool,
    /// When true with interception, unmatched requests Pause until forward/drop/timeout.
    breakpoint_mode: AtomicBool,
    pending_interceptions: RwLock<Vec<InterceptedRequest>>,
    /// Resolutions for Pause'd request ids (set by forward_request / drop_request).
    pause_releases: Mutex<HashMap<String, PauseRelease>>,
    pause_cv: Condvar,
    history: RwLock<Vec<(InterceptedRequest, Option<InterceptedResponse>)>>,
    rules: RwLock<Vec<TamperRule>>,
}

impl Default for TamperEngineInner {
    fn default() -> Self {
        Self {
            interception_enabled: AtomicBool::new(false),
            breakpoint_mode: AtomicBool::new(false),
            pending_interceptions: RwLock::new(Vec::new()),
            pause_releases: Mutex::new(HashMap::new()),
            pause_cv: Condvar::new(),
            history: RwLock::new(Vec::new()),
            rules: RwLock::new(Vec::new()),
        }
    }
}

/// Central Interceptor & Repeater engine for Thor
#[derive(Debug, Clone, Default)]
pub struct TrafficTamperEngine {
    inner: Arc<TamperEngineInner>,
}

impl TrafficTamperEngine {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TamperEngineInner::default()),
        }
    }

    /// Toggle live traffic interception state
    pub fn set_interception_enabled(&self, enabled: bool) {
        self.inner
            .interception_enabled
            .store(enabled, Ordering::Release);
        // Enabling intercept implies breakpoint hold until UI forward/drop (or timeout).
        self.inner.breakpoint_mode.store(enabled, Ordering::Release);
    }

    pub fn is_interception_enabled(&self) -> bool {
        self.inner.interception_enabled.load(Ordering::Acquire)
    }

    pub fn set_breakpoint_mode(&self, enabled: bool) {
        self.inner.breakpoint_mode.store(enabled, Ordering::Release);
    }

    pub fn is_breakpoint_mode(&self) -> bool {
        self.inner.breakpoint_mode.load(Ordering::Acquire)
    }

    /// Process an outgoing request through rules and interception pipeline
    pub fn process_outgoing_request(&self, req: InterceptedRequest) -> Option<InterceptedRequest> {
        match self.evaluate_tamper_outcome(req) {
            TamperOutcome::Forward(r) => Some(r),
            TamperOutcome::Mock(_) | TamperOutcome::Pause => None,
        }
    }

    /// Get all pending intercepted requests waiting for manual tampering
    pub fn get_pending_requests(&self) -> Vec<InterceptedRequest> {
        self.inner
            .pending_interceptions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Forward a pending request (with optional manual edits)
    pub fn forward_request(
        &self,
        request_id: &str,
        edited_req: Option<InterceptedRequest>,
    ) -> Option<InterceptedRequest> {
        let mut pending = self
            .inner
            .pending_interceptions
            .write()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = pending.iter().position(|r| r.id == request_id) {
            let req = pending.remove(pos);
            let out = edited_req.unwrap_or(req);
            {
                let mut releases = self
                    .inner
                    .pause_releases
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                releases.insert(request_id.to_string(), PauseRelease::Forward(out.clone()));
            }
            self.inner.pause_cv.notify_all();
            return Some(out);
        }
        None
    }

    /// Drop/cancel an intercepted request without forwarding
    pub fn drop_request(&self, request_id: &str) -> bool {
        let mut pending = self
            .inner
            .pending_interceptions
            .write()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = pending.iter().position(|r| r.id == request_id) {
            let req = pending.remove(pos);
            self.record_history(req, None);
            {
                let mut releases = self
                    .inner
                    .pause_releases
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                releases.insert(request_id.to_string(), PauseRelease::Drop);
            }
            self.inner.pause_cv.notify_all();
            return true;
        }
        false
    }

    /// Block until UI forwards/drops a Pause'd request, or `timeout` elapses.
    /// On timeout, auto-continues with the original pending request (removed from queue).
    pub fn wait_for_pause_release(&self, request_id: &str, timeout: Duration) -> PauseRelease {
        let deadline = Instant::now() + timeout;
        let mut releases = self
            .inner
            .pause_releases
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(action) = releases.remove(request_id) {
                return action;
            }
            let now = Instant::now();
            if now >= deadline {
                // Timeout: pull from pending and forward as-is so the tab unsticks.
                drop(releases);
                let mut pending = self
                    .inner
                    .pending_interceptions
                    .write()
                    .unwrap_or_else(|e| e.into_inner());
                if let Some(pos) = pending.iter().position(|r| r.id == request_id) {
                    let req = pending.remove(pos);
                    return PauseRelease::Forward(req);
                }
                return PauseRelease::Drop;
            }
            let wait = deadline - now;
            let (guard, _) = self
                .inner
                .pause_cv
                .wait_timeout(releases, wait)
                .unwrap_or_else(|e| e.into_inner());
            releases = guard;
        }
    }

    /// Record completed request/response pair into history for Repeater/Inspector (with SEC-027 secret redaction)
    pub fn record_history(
        &self,
        mut req: InterceptedRequest,
        mut resp: Option<InterceptedResponse>,
    ) {
        for (k, v) in req.headers.iter_mut() {
            let k_lower = k.to_lowercase();
            if k_lower == "authorization" || k_lower == "cookie" || k_lower == "x-api-key" {
                *v = "[REDACTED]".to_string();
            }
        }
        if let Some(ref mut res) = resp {
            for (k, v) in res.headers.iter_mut() {
                let k_lower = k.to_lowercase();
                if k_lower == "set-cookie" || k_lower == "authorization" {
                    *v = "[REDACTED]".to_string();
                }
            }
        }
        let mut hist = self
            .inner
            .history
            .write()
            .unwrap_or_else(|e| e.into_inner());
        hist.push((req, resp));
    }

    pub fn get_history(&self) -> Vec<(InterceptedRequest, Option<InterceptedResponse>)> {
        self.inner
            .history
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Add automated tamper rule
    pub fn add_rule(&self, rule: TamperRule) {
        if let Ok(mut rules) = self.inner.rules.write() {
            rules.push(rule);
        }
    }

    /// Retrieve a specific request by ID from pending or history
    pub fn get_request_by_id(&self, request_id: &str) -> Option<InterceptedRequest> {
        if let Ok(pending) = self.inner.pending_interceptions.read() {
            if let Some(req) = pending.iter().find(|r| r.id == request_id) {
                return Some(req.clone());
            }
        }
        if let Ok(hist) = self.inner.history.read() {
            if let Some((req, _)) = hist.iter().find(|(r, _)| r.id == request_id) {
                return Some(req.clone());
            }
        }
        None
    }

    /// Retrieve the most recent recorded or intercepted request
    pub fn get_latest_request(&self) -> Option<InterceptedRequest> {
        if let Ok(pending) = self.inner.pending_interceptions.read() {
            if let Some(req) = pending.last() {
                return Some(req.clone());
            }
        }
        if let Ok(hist) = self.inner.history.read() {
            if let Some((req, _)) = hist.last() {
                return Some(req.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: &str, timestamp: u64, method: &str, url: &str) -> InterceptedRequest {
        InterceptedRequest {
            id: id.to_string(),
            timestamp,
            method: method.to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
            body: None,
            is_modified: false,
        }
    }

    #[test]
    fn test_interceptor_toggle_and_process() {
        let engine = TrafficTamperEngine::new();
        assert!(!engine.is_interception_enabled());

        let req = request("req_1", 1000, "GET", "https://api.example.com/data");

        // When disabled, request passes straight through
        let processed = engine.process_outgoing_request(req.clone());
        assert!(processed.is_some());

        // Enable interception
        engine.set_interception_enabled(true);
        let paused = engine.process_outgoing_request(req.clone());
        assert!(paused.is_none());
        assert_eq!(engine.get_pending_requests().len(), 1);

        // Forward request
        let forwarded = engine.forward_request("req_1", None);
        assert!(forwarded.is_some());
        assert_eq!(engine.get_pending_requests().len(), 0);
    }

    #[test]
    fn test_automated_tamper_rule() {
        let engine = TrafficTamperEngine::new();
        // Later rules can match headers inserted by earlier rules.
        for (id, required_header, key, value) in [
            ("rule_1", None, "X-Thor-Injected", "true"),
            (
                "rule_2",
                Some("X-Thor-Injected"),
                "Authorization",
                "Bearer dependent",
            ),
        ] {
            engine.add_rule(TamperRule {
                id: id.to_string(),
                enabled: true,
                target_url_pattern: "api.example.com".to_string(),
                match_header_key: required_header.map(str::to_owned),
                replace_header_key: key.to_string(),
                replace_header_value: value.to_string(),
                mock_status: None,
                mock_body: None,
                delay_ms: None,
            });
        }
        let req = request("req_2", 1000, "POST", "https://api.example.com/v1/user");

        let processed = engine.process_outgoing_request(req).unwrap();
        assert!(processed.is_modified);
        assert_eq!(
            processed.headers.get("X-Thor-Injected").map(|s| s.as_str()),
            Some("true")
        );
        assert_eq!(
            processed.headers.get("Authorization").map(String::as_str),
            Some("Bearer dependent")
        );
    }

    #[test]
    fn test_drop_request() {
        let engine = TrafficTamperEngine::new();
        engine.set_interception_enabled(true);
        let req = request("req_drop", 1000, "GET", "https://api.example.com/drop");
        engine.process_outgoing_request(req);
        assert_eq!(engine.get_pending_requests().len(), 1);

        assert!(engine.drop_request("req_drop"));
        assert_eq!(engine.get_pending_requests().len(), 0);
    }

    #[test]
    fn test_pause_release_forward_unblocks() {
        let engine = TrafficTamperEngine::new();
        engine.set_interception_enabled(true);
        assert!(engine.is_breakpoint_mode());
        let req = request("req_hold", 1, "GET", "https://example.test/hold");
        assert!(matches!(
            engine.evaluate_tamper_outcome(req),
            TamperOutcome::Pause
        ));
        let engine2 = engine.clone();
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            engine2.forward_request("req_hold", None);
        });
        let release = engine.wait_for_pause_release("req_hold", Duration::from_secs(2));
        assert!(matches!(release, PauseRelease::Forward(_)));
        h.join().unwrap();
    }
}
