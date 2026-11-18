use super::{HashMap, NetworkTrace, PropertyCase, QaEngine};

impl QaEngine {
    /// Infer property / idempotency / pagination / authz generative cases from
    /// recorded network traces.
    pub fn mine_trace_properties(&self, traces: &[NetworkTrace]) -> Vec<PropertyCase> {
        let mut cases = Vec::new();
        let mut by_key: HashMap<(String, String), Vec<&NetworkTrace>> = HashMap::new();
        for t in traces {
            let path = t.url.clone();
            by_key
                .entry((t.method.to_uppercase(), path))
                .or_default()
                .push(t);
        }
        for ((method, url), group) in &by_key {
            if method == "POST" || method == "PUT" || method == "PATCH" {
                cases.push(PropertyCase {
                    name: format!("idempotency:{method}:{url}"),
                    kind: "idempotency".into(),
                    description: format!(
                        "Replay {method} {url} with Idempotency-Key; expect stable status"
                    ),
                    seed_input: serde_json::json!({
                        "method": method,
                        "url": url,
                        "sample_status": group.first().map(|t| t.status),
                    }),
                });
            }
            let lower = url.to_lowercase();
            if lower.contains("page=")
                || lower.contains("cursor=")
                || lower.contains("offset=")
                || lower.contains("/next")
            {
                cases.push(PropertyCase {
                    name: format!("pagination:{url}"),
                    kind: "pagination".into(),
                    description: format!("Walk pagination params on {url}"),
                    seed_input: serde_json::json!({ "url": url, "method": method }),
                });
            }
            if lower.contains("/admin")
                || lower.contains("/internal")
                || group.iter().any(|t| {
                    t.headers.keys().any(|k| {
                        k.eq_ignore_ascii_case("authorization")
                            || k.eq_ignore_ascii_case("x-api-key")
                    })
                })
            {
                cases.push(PropertyCase {
                    name: format!("authz:{method}:{url}"),
                    kind: "authz".into(),
                    description: format!("Probe {method} {url} across identities"),
                    seed_input: serde_json::json!({ "url": url, "method": method }),
                });
            }
            if group.len() >= 2 {
                let statuses: Vec<_> = group.iter().map(|t| t.status).collect();
                cases.push(PropertyCase {
                    name: format!("property:status-stability:{url}"),
                    kind: "property".into(),
                    description: format!("Status multiset across repeats: {statuses:?}"),
                    seed_input: serde_json::json!({ "statuses": statuses }),
                });
            }
        }
        cases
    }
}
