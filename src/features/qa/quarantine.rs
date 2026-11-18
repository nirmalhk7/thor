use super::{HashMap, QaEngine, QuarantineDecision, QuarantineInput, TestGenerator};

impl QaEngine {
    /// Load quarantine.yaml into the in-memory map (best-effort).
    pub(super) fn hydrate_quarantine(&self) {
        let Ok(yaml) = Self::load_quarantine_yaml() else {
            return;
        };
        self.ingest_quarantine_yaml(&yaml);
    }

    /// Parse a quarantine.yaml body into the in-memory map.
    pub fn ingest_quarantine_yaml(&self, yaml: &str) {
        let mut map = self.quarantine.lock();
        for row in quarantine_rows(yaml) {
            map.insert(row.test.clone(), hydrated_decision(row));
        }
    }

    /// Decide quarantine / skip / expire / re-admit for a flaky test.
    /// Persists the in-memory quarantine map to `thor_data_dir()/quarantine.yaml`.
    ///
    /// Persist outcome is never swallowed: `persist_ok` / `persist_err` report
    /// whether `quarantine.yaml` was written.
    pub fn evaluate_quarantine(&self, input: &QuarantineInput) -> QuarantineDecision {
        let action = if input.currently_quarantined {
            if input.green_streak >= 5 {
                "readmit"
            } else if input.days_in_quarantine >= 14.0 {
                "expire"
            } else if input.flake_rate >= 0.5 {
                "skip"
            } else {
                "keep"
            }
        } else if input.flake_rate >= 0.3 || input.consecutive_fails >= 3 {
            "quarantine"
        } else {
            "keep"
        };
        let reason = match action {
            "quarantine" => format!(
                "flake_rate={:.2} consecutive_fails={}",
                input.flake_rate, input.consecutive_fails
            ),
            "skip" => "still highly flaky while quarantined".into(),
            "expire" => format!(
                "quarantine aged out after {:.1} days",
                input.days_in_quarantine
            ),
            "readmit" => format!("green streak of {}", input.green_streak),
            _ => "within policy thresholds".into(),
        };
        let yaml = format!(
            "quarantine:\n  - test: \"{}\"\n    action: {action}\n    reason: \"{}\"\n",
            input.test.replace('"', "\\\""),
            reason.replace('"', "\\\"")
        );
        let mut decision = QuarantineDecision {
            test: input.test.clone(),
            action: action.to_string(),
            reason,
            quarantine_yaml: yaml,
            persist_ok: false,
            persist_err: None,
        };
        self.quarantine
            .lock()
            .insert(input.test.clone(), decision.clone());
        match self.persist_quarantine() {
            Ok(_) => {
                decision.persist_ok = true;
                decision.persist_err = None;
            }
            Err(e) => {
                decision.persist_ok = false;
                decision.persist_err = Some(e);
            }
        }
        // Keep map entry in sync with persist outcome.
        self.quarantine
            .lock()
            .insert(input.test.clone(), decision.clone());
        decision
    }

    pub fn quarantine_status(&self, test: &str) -> Option<QuarantineDecision> {
        self.quarantine.lock().get(test).cloned()
    }

    /// Path to the on-disk quarantine policy (`thor_data_dir()/quarantine.yaml`).
    pub fn quarantine_path() -> std::path::PathBuf {
        crate::paths::thor_quarantine_path()
    }

    /// Write the in-memory quarantine map as YAML under `thor_quarantine_path()`.
    ///
    /// Merges with any existing on-disk entries so a process-wide engine that only
    /// knows about a subset of tests cannot clobber unrelated quarantine rows
    /// (important when tests share `THOR_DATA_DIR`).
    pub fn persist_quarantine(&self) -> Result<std::path::PathBuf, String> {
        let path = Self::quarantine_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create quarantine dir: {e}"))?;
        }
        let mut entries: std::collections::BTreeMap<String, (String, String)> =
            std::collections::BTreeMap::new();
        if let Ok(existing) = Self::load_quarantine_yaml() {
            seed_quarantine_entries(&mut entries, &existing);
        }
        {
            let map = self.quarantine.lock();
            merge_quarantine_decisions(&mut entries, &map);
        }
        let yaml = if entries.is_empty() {
            String::from("quarantine: []\n")
        } else {
            let list: Vec<(String, String, String)> = entries
                .into_iter()
                .map(|(test, (action, reason))| (test, action, reason))
                .collect();
            TestGenerator::quarantine_yaml(&list)
        };
        std::fs::write(&path, &yaml).map_err(|e| format!("write quarantine.yaml: {e}"))?;
        Ok(path)
    }

    /// Load quarantine.yaml from disk (best-effort; empty if missing).
    pub fn load_quarantine_yaml() -> Result<String, String> {
        let path = Self::quarantine_path();
        if !path.exists() {
            return Ok(String::from("quarantine: []\n"));
        }
        std::fs::read_to_string(&path).map_err(|e| format!("read quarantine.yaml: {e}"))
    }

    /// CI filter snippet for currently quarantined / skipped tests.
    /// Never empty — falls back to the stub from [`TestGenerator::ci_quarantine_filter`].
    pub fn ci_quarantine_filter(&self) -> String {
        let map = self.quarantine.lock();
        let titles: Vec<String> = map
            .values()
            .filter(|d| matches!(d.action.as_str(), "quarantine" | "skip"))
            .map(|d| d.test.clone())
            .collect();
        let filter = TestGenerator::ci_quarantine_filter(&titles);
        if filter.trim().is_empty() {
            TestGenerator::ci_quarantine_filter(&[])
        } else {
            filter
        }
    }
}

struct QuarantineRow {
    test: String,
    action: String,
    reason: String,
}

struct QuarantineRows<'a> {
    lines: std::str::Lines<'a>,
    test: Option<String>,
    action: Option<String>,
    reason: String,
}

fn quarantine_rows(yaml: &str) -> QuarantineRows<'_> {
    QuarantineRows {
        lines: yaml.lines(),
        test: None,
        action: None,
        reason: String::new(),
    }
}

impl QuarantineRows<'_> {
    fn take_row(&mut self) -> Option<QuarantineRow> {
        match (self.test.take(), self.action.take()) {
            (Some(test), Some(action)) => Some(QuarantineRow {
                test,
                action,
                reason: self.reason.clone(),
            }),
            _ => None,
        }
    }
}

impl Iterator for QuarantineRows<'_> {
    type Item = QuarantineRow;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(line) = self.lines.next() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("- test:") {
                let row = self.take_row();
                self.reason.clear();
                self.test = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
                if row.is_some() {
                    return row;
                }
            } else if let Some(rest) = trimmed.strip_prefix("action:") {
                self.action = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed.strip_prefix("reason:") {
                self.reason = rest.trim().trim_matches('"').trim_matches('\'').to_string();
            }
        }
        self.take_row()
    }
}

fn hydrated_decision(row: QuarantineRow) -> QuarantineDecision {
    let QuarantineRow {
        test,
        action,
        reason,
    } = row;
    let quarantine_yaml = format!(
        "quarantine:\n  - test: \"{}\"\n    action: {action}\n    reason: \"{}\"\n",
        test.replace('"', "\\\""),
        reason.replace('"', "\\\"")
    );
    QuarantineDecision {
        test,
        action,
        reason,
        quarantine_yaml,
        persist_ok: true,
        persist_err: None,
    }
}

fn seed_quarantine_entries(
    entries: &mut std::collections::BTreeMap<String, (String, String)>,
    yaml: &str,
) {
    for row in quarantine_rows(yaml) {
        if matches!(row.action.as_str(), "quarantine" | "skip" | "keep") {
            entries.insert(row.test, (row.action, row.reason));
        }
    }
}

fn merge_quarantine_decisions(
    entries: &mut std::collections::BTreeMap<String, (String, String)>,
    decisions: &HashMap<String, QuarantineDecision>,
) {
    for decision in decisions.values() {
        match decision.action.as_str() {
            "quarantine" | "skip" | "keep" => {
                entries.insert(
                    decision.test.clone(),
                    (decision.action.clone(), decision.reason.clone()),
                );
            }
            "expire" | "readmit" => {
                entries.remove(&decision.test);
            }
            _ => {}
        }
    }
}
