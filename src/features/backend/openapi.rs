use super::{
    ActualInteraction, BTreeMap, BackendEngine, OpenApiDiffReport, OpenApiPath,
    OpenApiTrafficReport,
};

impl BackendEngine {
    pub fn openapi_diff_gate(
        &self,
        before: &[OpenApiPath],
        after: &[OpenApiPath],
    ) -> OpenApiDiffReport {
        let key = |p: &OpenApiPath| format!("{} {}", p.method.to_uppercase(), p.path);
        let bset: BTreeMap<String, &OpenApiPath> = before.iter().map(|p| (key(p), p)).collect();
        let aset: BTreeMap<String, &OpenApiPath> = after.iter().map(|p| (key(p), p)).collect();
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();
        for (k, p) in &aset {
            if !bset.contains_key(k) {
                added.push(k.clone());
            } else if bset[k].operation_id != p.operation_id {
                changed.push(format!("{k} operationId drift"));
            }
        }
        for k in bset.keys() {
            if !aset.contains_key(k) {
                removed.push(k.clone());
            }
        }
        let breaking = !removed.is_empty();
        OpenApiDiffReport {
            added,
            removed,
            changed,
            breaking,
        }
    }

    /// Parse OpenAPI 3.x / Swagger 2 JSON into path+method rows.
    pub fn parse_openapi_paths(doc: &serde_json::Value) -> Vec<OpenApiPath> {
        let mut out = Vec::new();
        let paths = match doc.get("paths").and_then(|p| p.as_object()) {
            Some(p) => p,
            None => return out,
        };
        for (path, item) in paths {
            let Some(obj) = item.as_object() else {
                continue;
            };
            for (method, op) in obj {
                let m = method.to_lowercase();
                if !matches!(
                    m.as_str(),
                    "get" | "post" | "put" | "patch" | "delete" | "head" | "options" | "trace"
                ) {
                    continue;
                }
                let operation_id = op
                    .get("operationId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                out.push(OpenApiPath {
                    path: path.clone(),
                    method: m.to_uppercase(),
                    operation_id,
                });
            }
        }
        out
    }

    /// Validate captured HTTP interactions against declared OpenAPI paths.
    pub fn openapi_validate_traffic(
        &self,
        paths: &[OpenApiPath],
        interactions: &[ActualInteraction],
    ) -> OpenApiTrafficReport {
        let declared: BTreeMap<String, &OpenApiPath> = paths
            .iter()
            .map(|p| {
                (
                    format!(
                        "{} {}",
                        p.method.to_uppercase(),
                        normalize_api_path(&p.path)
                    ),
                    p,
                )
            })
            .collect();
        let mut matched = 0usize;
        let mut unmatched = Vec::new();
        let mut undeclared_methods = Vec::new();
        for i in interactions {
            let path = normalize_api_path(&i.path);
            let key = format!("{} {}", i.method.to_uppercase(), path);
            if declared.contains_key(&key) {
                matched += 1;
            } else {
                // Same path, different method?
                let path_exists = paths.iter().any(|p| normalize_api_path(&p.path) == path);
                if path_exists {
                    undeclared_methods.push(key.clone());
                }
                unmatched.push(key);
            }
        }
        let ok = unmatched.is_empty();
        OpenApiTrafficReport {
            matched,
            unmatched,
            undeclared_methods,
            ok,
        }
    }
}
fn normalize_api_path(path: &str) -> String {
    let p = path.split('?').next().unwrap_or(path);
    if p.starts_with('/') {
        p.to_string()
    } else {
        format!("/{p}")
    }
}
