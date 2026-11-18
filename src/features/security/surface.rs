use super::{BTreeMap, SecurityEngine, SurfaceEndpoint};

impl SecurityEngine {
    // 6. Surface Crawler ----------------------------------------------------
    /// Collapse discovered (method, url) pairs into parameterized endpoint
    /// templates (numeric / uuid segments become `:id`) and aggregate params.
    pub fn crawl_surface(&self, discovered: &[(String, String)]) -> Vec<SurfaceEndpoint> {
        let mut map: BTreeMap<(String, String), (std::collections::BTreeSet<String>, usize)> =
            BTreeMap::new();
        for (method, url) in discovered {
            let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
            let template = templatize_path(path);
            let entry = map.entry((method.to_uppercase(), template)).or_default();
            for pair in query.split('&').filter(|p| !p.is_empty()) {
                let key = pair.split('=').next().unwrap_or("").to_string();
                if !key.is_empty() {
                    entry.0.insert(key);
                }
            }
            entry.1 += 1;
        }
        map.into_iter()
            .map(
                |((method, path_template), (params, instances))| SurfaceEndpoint {
                    method,
                    path_template,
                    params: params.into_iter().collect(),
                    instances,
                },
            )
            .collect()
    }

    /// Scope-filtered surface crawl: only (method, url) pairs whose host is in
    /// `host_allowlist` (or a subdomain of an allowlisted host) are templatized.
    /// An empty allowlist admits everything.
    pub fn crawl_surface_scoped(
        &self,
        discovered: &[(String, String)],
        host_allowlist: &[String],
    ) -> Vec<SurfaceEndpoint> {
        let in_scope = |url: &str| {
            if host_allowlist.is_empty() {
                return true;
            }
            let host = host_of(url);
            host_allowlist.iter().any(|h| {
                let h = h.to_lowercase();
                host == h || host.ends_with(&format!(".{h}"))
            })
        };
        let filtered: Vec<(String, String)> = discovered
            .iter()
            .filter(|(_, url)| in_scope(url))
            .cloned()
            .collect();
        self.crawl_surface(&filtered)
    }
}
fn templatize_path(path: &str) -> String {
    let segs: Vec<String> = path
        .split('/')
        .map(|s| {
            if s.is_empty() {
                s.to_string()
            } else if s.chars().all(|c| c.is_ascii_digit()) {
                ":id".to_string()
            } else if is_uuid(s) {
                ":uuid".to_string()
            } else if s.len() >= 24 && s.chars().all(|c| c.is_ascii_alphanumeric()) {
                ":token".to_string()
            } else {
                s.to_string()
            }
        })
        .collect();
    segs.join("/")
}

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.iter().map(|part| part.len()).eq([8, 4, 4, 4, 12])
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Extract the lowercased host of a URL (no scheme/port/path).
fn host_of(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    after
        .split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_lowercase()
}
