use super::{
    InterceptedRequest, InterceptedResponse, TamperOutcome, TamperRule, TrafficTamperEngine,
};

impl TrafficTamperEngine {
    /// Process an outgoing request returning detailed TamperOutcome enum
    pub fn evaluate_tamper_outcome(&self, mut req: InterceptedRequest) -> TamperOutcome {
        let rules = self.inner.rules.read().unwrap_or_else(|e| e.into_inner());
        for rule in rules.iter().filter(|r| r.enabled) {
            if !rule.matches_request(&req) {
                continue;
            }
            rule.apply_header(&mut req);
            if let Some(mock_resp) = rule.mock_response(&req) {
                self.record_history(req.clone(), Some(mock_resp.clone()));
                return TamperOutcome::Mock(mock_resp);
            }
        }
        // If interception is active, queue request for manual review/modification
        if self.is_interception_enabled() {
            if self.is_breakpoint_mode() {
                let mut pending = self
                    .inner
                    .pending_interceptions
                    .write()
                    .unwrap_or_else(|e| e.into_inner());
                pending.push(req);
                TamperOutcome::Pause
            } else {
                // Observe mode: do not block; Fetch handler records history on Forward.
                TamperOutcome::Forward(req)
            }
        } else {
            TamperOutcome::Forward(req)
        }
    }
}

impl TamperRule {
    fn matches_request(&self, req: &InterceptedRequest) -> bool {
        req.url.contains(&self.target_url_pattern)
            && self
                .match_header_key
                .as_ref()
                .is_none_or(|key| req.headers.contains_key(key))
    }

    fn apply_header(&self, req: &mut InterceptedRequest) {
        if !self.replace_header_key.is_empty() {
            req.headers.insert(
                self.replace_header_key.clone(),
                self.replace_header_value.clone(),
            );
            req.is_modified = true;
        }
    }

    fn mock_response(&self, req: &InterceptedRequest) -> Option<InterceptedResponse> {
        if self.mock_status.is_none() && self.mock_body.is_none() {
            return None;
        }
        Some(InterceptedResponse {
            request_id: req.id.clone(),
            timestamp: req.timestamp,
            status_code: self.mock_status.unwrap_or(200),
            headers: [("content-type".to_string(), "application/json".to_string())]
                .into_iter()
                .collect(),
            body: self.mock_body.clone(),
            duration_ms: self.delay_ms.unwrap_or(0),
        })
    }
}
