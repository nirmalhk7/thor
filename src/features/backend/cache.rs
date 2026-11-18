use super::{BackendEngine, CacheSemanticsEntry, CacheSemanticsReport};

impl BackendEngine {
    pub fn cache_semantics_lens(&self, entries: &[CacheSemanticsEntry]) -> CacheSemanticsReport {
        let mut findings = Vec::new();
        let mut cacheable = 0usize;
        let mut non_cacheable = 0usize;
        for e in entries {
            let cc = e.cache_control.to_lowercase();
            let no_store = cc.contains("no-store") || cc.contains("private");
            let method_ok = matches!(e.method.to_uppercase().as_str(), "GET" | "HEAD");
            if no_store || !method_ok || e.status >= 400 {
                non_cacheable += 1;
                if method_ok && e.status < 400 && cc.is_empty() {
                    findings.push(format!("{} missing Cache-Control", e.url));
                }
            } else {
                cacheable += 1;
                if e.etag.is_none() && !cc.contains("max-age") {
                    findings.push(format!("{} cacheable without ETag or max-age", e.url));
                }
            }
            if let Some(v) = &e.vary {
                if v.to_lowercase().contains("*") {
                    findings.push(format!("{} Vary: * disables shared caches", e.url));
                }
            }
        }
        CacheSemanticsReport {
            findings,
            cacheable,
            non_cacheable,
        }
    }
}
