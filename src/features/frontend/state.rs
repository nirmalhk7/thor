use super::{now_ms, FrontendEngine, StateDiffEntry, StateSnapshot};

impl FrontendEngine {
    // 6. State Time-Travel --------------------------------------------------
    pub fn record_state(&self, label: &str, state: serde_json::Value) -> usize {
        let mut snaps = self.snapshots.lock();
        let index = snaps.len();
        let timestamp_ms = now_ms();
        snaps.push(StateSnapshot {
            index,
            label: label.to_string(),
            timestamp_ms,
            state,
        });
        index
    }

    pub fn state_snapshots(&self) -> Vec<StateSnapshot> {
        self.snapshots.lock().clone()
    }

    pub fn clear_state(&self) {
        self.snapshots.lock().clear();
    }

    /// Restore (return) the recorded state at `index` so a caller can apply it
    /// back into the running app (time-travel jump). `None` when out of range.
    pub fn restore_state(&self, index: usize) -> Option<serde_json::Value> {
        self.snapshots.lock().get(index).map(|s| s.state.clone())
    }

    pub fn state_diff(&self, from: usize, to: usize) -> Vec<StateDiffEntry> {
        let snaps = self.snapshots.lock();
        match (snaps.get(from), snaps.get(to)) {
            (Some(a), Some(b)) => {
                let mut out = Vec::new();
                Self::json_diff("", &a.state, &b.state, &mut out);
                out
            }
            _ => Vec::new(),
        }
    }

    fn json_diff(
        path: &str,
        a: &serde_json::Value,
        b: &serde_json::Value,
        out: &mut Vec<StateDiffEntry>,
    ) {
        use serde_json::Value;
        match (a, b) {
            (Value::Object(oa), Value::Object(ob)) => {
                let mut keys: Vec<&String> = oa.keys().chain(ob.keys()).collect();
                keys.sort();
                keys.dedup();
                for k in keys {
                    let child = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    match (oa.get(k), ob.get(k)) {
                        (Some(va), Some(vb)) => Self::json_diff(&child, va, vb, out),
                        (Some(va), None) => out.push(StateDiffEntry {
                            path: child,
                            before: Some(va.clone()),
                            after: None,
                        }),
                        (None, Some(vb)) => out.push(StateDiffEntry {
                            path: child,
                            before: None,
                            after: Some(vb.clone()),
                        }),
                        (None, None) => {}
                    }
                }
            }
            _ => {
                if a != b {
                    out.push(StateDiffEntry {
                        path: path.to_string(),
                        before: Some(a.clone()),
                        after: Some(b.clone()),
                    });
                }
            }
        }
    }
}
