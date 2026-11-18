use super::{BTreeMap, BackendEngine, MultipartPart, MultipartReport};

impl BackendEngine {
    pub fn multipart_workbench(&self, parts: &[MultipartPart]) -> MultipartReport {
        let mut issues = Vec::new();
        let total_bytes: usize = parts.iter().map(|p| p.size).sum();
        let mut names = BTreeMap::<String, usize>::new();
        for p in parts {
            *names.entry(p.name.clone()).or_default() += 1;
            if p.filename.is_some() && p.content_type.is_none() {
                issues.push(format!("part '{}' file missing content-type", p.name));
            }
            if p.size > 10_000_000 {
                issues.push(format!("part '{}' exceeds 10MB ({})", p.name, p.size));
            }
            if p.name.is_empty() {
                issues.push("unnamed multipart part".into());
            }
        }
        for (n, c) in names {
            if c > 1 {
                issues.push(format!("duplicate part name '{n}' x{c}"));
            }
        }
        MultipartReport {
            parts: parts.len(),
            total_bytes,
            issues,
        }
    }
}
