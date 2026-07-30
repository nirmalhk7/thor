use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct EvidenceEnvelope<T> {
    pub timestamp_ms: u128,
    pub tab_id: Option<u64>,
    pub data: T,
}

impl<T> EvidenceEnvelope<T> {
    pub fn new(data: T, tab_id: Option<u64>) -> Self {
        Self {
            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
            tab_id,
            data,
        }
    }
}

pub struct ResourceBudgetController {
    dropped_count: usize,
    truncated_count: usize,
}

impl ResourceBudgetController {
    pub fn new() -> Self {
        Self { dropped_count: 0, truncated_count: 0 }
    }
    
    pub fn report_drop(&mut self) { self.dropped_count += 1; }
    pub fn report_truncate(&mut self) { self.truncated_count += 1; }
    
    pub fn get_dropped(&self) -> usize { self.dropped_count }
    pub fn get_truncated(&self) -> usize { self.truncated_count }
}

pub struct EvidenceStore {
    history: VecDeque<EvidenceEnvelope<String>>,
    console: VecDeque<EvidenceEnvelope<String>>,
    network: VecDeque<EvidenceEnvelope<String>>,
    trace: VecDeque<EvidenceEnvelope<String>>,
    tamper: VecDeque<EvidenceEnvelope<String>>,
    screenshot_bytes: usize,
    budget: ResourceBudgetController,
}

impl EvidenceStore {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(100),
            console: VecDeque::with_capacity(1000),
            network: VecDeque::with_capacity(2000),
            trace: VecDeque::with_capacity(5000),
            tamper: VecDeque::with_capacity(1000),
            screenshot_bytes: 0,
            budget: ResourceBudgetController::new(),
        }
    }

    pub fn redact_secrets(headers: &mut HashMap<String, String>) {
        let sensitive = ["authorization", "cookie", "set-cookie", "x-api-key"];
        for key in sensitive {
            // Keys are expected normalized lowercase by callers.
            if headers.contains_key(key) {
                headers.insert(key.to_string(), "[REDACTED]".to_string());
            }
        }
    }

    fn push_bounded<T>(queue: &mut VecDeque<EvidenceEnvelope<T>>, limit: usize, item: EvidenceEnvelope<T>, budget: &mut ResourceBudgetController) {
        if queue.len() >= limit {
            queue.pop_front();
            budget.report_drop();
        }
        queue.push_back(item);
    }

    pub fn add_history(&mut self, item: EvidenceEnvelope<String>) {
        Self::push_bounded(&mut self.history, 100, item, &mut self.budget);
    }

    pub fn add_console(&mut self, item: EvidenceEnvelope<String>) {
        Self::push_bounded(&mut self.console, 1000, item, &mut self.budget);
    }

    pub fn add_network(&mut self, item: EvidenceEnvelope<String>) {
        Self::push_bounded(&mut self.network, 2000, item, &mut self.budget);
    }

    pub fn add_trace(&mut self, item: EvidenceEnvelope<String>) {
        Self::push_bounded(&mut self.trace, 5000, item, &mut self.budget);
    }

    pub fn add_tamper(&mut self, item: EvidenceEnvelope<String>) {
        Self::push_bounded(&mut self.tamper, 1000, item, &mut self.budget);
    }

    pub fn add_screenshot(&mut self, size: usize) {
        if self.screenshot_bytes + size > 5 * 1024 * 1024 {
            self.budget.report_drop();
        } else {
            self.screenshot_bytes += size;
        }
    }

    pub fn diagnostics_json(&self) -> String {
        format!(
            r#"{{"history": {}, "console": {}, "network": {}, "trace": {}, "tamper": {}, "screenshot_bytes": {}, "dropped": {}, "truncated": {}}}"#,
            self.history.len(),
            self.console.len(),
            self.network.len(),
            self.trace.len(),
            self.tamper.len(),
            self.screenshot_bytes,
            self.budget.get_dropped(),
            self.budget.get_truncated()
        )
    }
}

impl Default for EvidenceStore {
    fn default() -> Self {
        Self::new()
    }
}
