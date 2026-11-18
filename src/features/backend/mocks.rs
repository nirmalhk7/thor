use super::{
    BackendEngine, HashMap, InterceptedRequest, InterceptedResponse, MockMatch, MockRoute,
};

impl BackendEngine {
    /// Convert recorded tamper-engine traffic history into replayable mock routes
    /// (one per request/response pair). Returns the number of mocks added.
    pub fn mock_from_tamper_history(
        &self,
        history: &[(InterceptedRequest, Option<InterceptedResponse>)],
    ) -> usize {
        let mut added = 0;
        for (req, resp) in history {
            if let Some(r) = resp {
                self.add_mock(MockRoute {
                    method: req.method.clone(),
                    path_pattern: url_path(&req.url),
                    status: r.status_code,
                    body: r.body.clone().unwrap_or_default(),
                });
                added += 1;
            }
        }
        added
    }

    // 9. Mock / Stub --------------------------------------------------------
    pub fn add_mock(&self, route: MockRoute) {
        self.mocks.lock().push(route);
    }

    pub fn clear_mocks(&self) {
        self.mocks.lock().clear();
    }

    pub fn match_mock(&self, method: &str, path: &str) -> Option<MockMatch> {
        for route in self.mocks.lock().iter() {
            if !route.method.eq_ignore_ascii_case(method) {
                continue;
            }
            if let Some(params) = match_path(&route.path_pattern, path) {
                return Some(MockMatch {
                    status: route.status,
                    body: route.body.clone(),
                    path_params: params,
                });
            }
        }
        None
    }
}
/// Extract the path (no scheme/host/query) from a URL, defaulting to `/`.
fn url_path(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    let path = after.split('/').skip(1).collect::<Vec<_>>().join("/");
    let path = path.split('?').next().unwrap_or("");
    format!("/{path}")
}

/// Match `/users/:id` style pattern to a concrete path, returning captured params.
fn match_path(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
    let p_path = path.split('?').next().unwrap_or(path);
    let pat: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let seg: Vec<&str> = p_path.trim_matches('/').split('/').collect();
    if pat.len() != seg.len() {
        return None;
    }
    let mut params = HashMap::new();
    for (p, s) in pat.iter().zip(seg.iter()) {
        if let Some(name) = p.strip_prefix(':') {
            params.insert(name.to_string(), s.to_string());
        } else if p != s {
            return None;
        }
    }
    Some(params)
}
