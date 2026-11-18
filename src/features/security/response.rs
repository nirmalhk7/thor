use super::{HttpSnapshot, ResponseDiffReport, SecurityEngine};

impl SecurityEngine {
    // 2. Response Diff ------------------------------------------------------
    pub fn diff_responses(&self, a: &HttpSnapshot, b: &HttpSnapshot) -> ResponseDiffReport {
        let status_changed = if a.status != b.status {
            Some((a.status, b.status))
        } else {
            None
        };

        let mut header_changes = Vec::new();
        let mut keys: Vec<&String> = a.headers.keys().chain(b.headers.keys()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            match (a.headers.get(k), b.headers.get(k)) {
                (Some(va), Some(vb)) if va != vb => {
                    header_changes.push(format!("~ {k}: {va} -> {vb}"))
                }
                (Some(va), None) => header_changes.push(format!("- {k}: {va}")),
                (None, Some(vb)) => header_changes.push(format!("+ {k}: {vb}")),
                _ => {}
            }
        }

        let a_lines: Vec<&str> = a.body.lines().collect();
        let b_lines: Vec<&str> = b.body.lines().collect();
        let a_set: std::collections::HashSet<&str> = a_lines.iter().copied().collect();
        let b_set: std::collections::HashSet<&str> = b_lines.iter().copied().collect();
        let added_lines: Vec<String> = b_lines
            .iter()
            .filter(|l| !a_set.contains(*l))
            .map(|s| s.to_string())
            .collect();
        let removed_lines: Vec<String> = a_lines
            .iter()
            .filter(|l| !b_set.contains(*l))
            .map(|s| s.to_string())
            .collect();

        let common = a_set.intersection(&b_set).count();
        let union = a_set.union(&b_set).count().max(1);
        let similarity = common as f64 / union as f64;

        ResponseDiffReport {
            status_changed,
            header_changes,
            added_lines,
            removed_lines,
            similarity,
        }
    }
}
