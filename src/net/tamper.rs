use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamperRule {
    pub id: String,
    pub enabled: bool,
    pub target_url_pattern: String,
    pub match_header_key: Option<String>,
    pub replace_header_key: String,
    pub replace_header_value: String,
}

/// Central Interceptor & Repeater engine for Thor
#[derive(Debug, Clone, Default)]
pub struct TrafficTamperEngine {
    interception_enabled: Arc<RwLock<bool>>,
    pending_interceptions: Arc<RwLock<Vec<InterceptedRequest>>>,
    history: Arc<RwLock<Vec<(InterceptedRequest, Option<InterceptedResponse>)>>>,
    rules: Arc<RwLock<Vec<TamperRule>>>,
}

impl TrafficTamperEngine {
    pub fn new() -> Self {
        Self {
            interception_enabled: Arc::new(RwLock::new(false)),
            pending_interceptions: Arc::new(RwLock::new(Vec::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            rules: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Toggle live traffic interception state
    pub fn set_interception_enabled(&self, enabled: bool) {
        if let Ok(mut lock) = self.interception_enabled.write() {
            *lock = enabled;
        }
    }

    pub fn is_interception_enabled(&self) -> bool {
        *self.interception_enabled.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Process an outgoing request through rules and interception pipeline
    pub fn process_outgoing_request(&self, mut req: InterceptedRequest) -> Option<InterceptedRequest> {
        // Apply match & replace rules
        if let Ok(rules) = self.rules.read() {
            for rule in rules.iter().filter(|r| r.enabled) {
                if req.url.contains(&rule.target_url_pattern) {
                    req.headers.insert(rule.replace_header_key.clone(), rule.replace_header_value.clone());
                    req.is_modified = true;
                }
            }
        }

        // If interception is active, queue request for manual review/modification
        if self.is_interception_enabled() {
            if let Ok(mut pending) = self.pending_interceptions.write() {
                pending.push(req);
            }
            None // Paused for user editing
        } else {
            Some(req)
        }
    }

    /// Get all pending intercepted requests waiting for manual tampering
    pub fn get_pending_requests(&self) -> Vec<InterceptedRequest> {
        self.pending_interceptions.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Forward a pending request (with optional manual edits)
    pub fn forward_request(&self, request_id: &str, edited_req: Option<InterceptedRequest>) -> Option<InterceptedRequest> {
        if let Ok(mut pending) = self.pending_interceptions.write() {
            if let Some(pos) = pending.iter().position(|r| r.id == request_id) {
                let req = pending.remove(pos);
                return Some(edited_req.unwrap_or(req));
            }
        }
        None
    }

    /// Drop/cancel an intercepted request without forwarding
    pub fn drop_request(&self, request_id: &str) -> bool {
        if let Ok(mut pending) = self.pending_interceptions.write() {
            if let Some(pos) = pending.iter().position(|r| r.id == request_id) {
                pending.remove(pos);
                return true;
            }
        }
        false
    }

    /// Record completed request/response pair into history for Repeater/Inspector
    pub fn record_history(&self, req: InterceptedRequest, resp: Option<InterceptedResponse>) {
        if let Ok(mut hist) = self.history.write() {
            hist.push((req, resp));
        }
    }

    pub fn get_history(&self) -> Vec<(InterceptedRequest, Option<InterceptedResponse>)> {
        self.history.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Add automated tamper rule
    pub fn add_rule(&self, rule: TamperRule) {
        if let Ok(mut rules) = self.rules.write() {
            rules.push(rule);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interceptor_toggle_and_process() {
        let engine = TrafficTamperEngine::new();
        assert!(!engine.is_interception_enabled());

        let req = InterceptedRequest {
            id: "req_1".to_string(),
            timestamp: 1000,
            method: "GET".to_string(),
            url: "https://api.example.com/data".to_string(),
            headers: HashMap::new(),
            body: None,
            is_modified: false,
        };

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
        engine.add_rule(TamperRule {
            id: "rule_1".to_string(),
            enabled: true,
            target_url_pattern: "api.example.com".to_string(),
            match_header_key: None,
            replace_header_key: "X-Thor-Injected".to_string(),
            replace_header_value: "true".to_string(),
        });

        let req = InterceptedRequest {
            id: "req_2".to_string(),
            timestamp: 1000,
            method: "POST".to_string(),
            url: "https://api.example.com/v1/user".to_string(),
            headers: HashMap::new(),
            body: None,
            is_modified: false,
        };

        let processed = engine.process_outgoing_request(req).unwrap();
        assert!(processed.is_modified);
        assert_eq!(processed.headers.get("X-Thor-Injected").map(|s| s.as_str()), Some("true"));
    }

    #[test]
    fn test_drop_request() {
        let engine = TrafficTamperEngine::new();
        engine.set_interception_enabled(true);
        let req = InterceptedRequest {
            id: "req_drop".to_string(),
            timestamp: 1000,
            method: "GET".to_string(),
            url: "https://api.example.com/drop".to_string(),
            headers: HashMap::new(),
            body: None,
            is_modified: false,
        };
        engine.process_outgoing_request(req);
        assert_eq!(engine.get_pending_requests().len(), 1);

        assert!(engine.drop_request("req_drop"));
        assert_eq!(engine.get_pending_requests().len(), 0);
    }
}
