use super::{BTreeMap, PerfEngine, ResourceTiming, ThirdPartyEntry};

impl PerfEngine {
    // 12. Third-Party Attribution ------------------------------------------
    pub fn attribute_third_party(
        &self,
        first_party: &str,
        resources: &[ResourceTiming],
    ) -> Vec<ThirdPartyEntry> {
        let fp = registrable_domain(first_party);
        let mut map: BTreeMap<String, ThirdPartyEntry> = BTreeMap::new();
        for r in resources {
            let host = url_host(&r.url);
            let domain = registrable_domain(&host);
            if domain == fp || domain.is_empty() {
                continue;
            }
            let entry = map.entry(domain.clone()).or_insert(ThirdPartyEntry {
                domain,
                requests: 0,
                bytes: 0,
                blocking_ms: 0.0,
            });
            entry.requests += 1;
            entry.bytes += r.bytes;
            if r.render_blocking {
                entry.blocking_ms += r.total_ms();
            }
        }
        let mut out: Vec<ThirdPartyEntry> = map.into_values().collect();
        out.sort_by_key(|item| std::cmp::Reverse(item.bytes));
        out
    }
}
fn url_host(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    after_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Approximate registrable domain (last two labels). Good enough for
/// first-vs-third-party classification of common hosts.
fn registrable_domain(host_or_url: &str) -> String {
    let host = if host_or_url.contains("://") {
        url_host(host_or_url)
    } else {
        host_or_url.to_string()
    };
    let labels: Vec<&str> = host.split('.').filter(|s| !s.is_empty()).collect();
    if labels.len() >= 2 {
        format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1])
    } else {
        host
    }
}
