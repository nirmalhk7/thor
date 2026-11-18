use super::{
    BackendEngine, ChaosOutcome, ChaosRule, HashMap, IdempotencyEvent, IdempotencyFinding,
};

impl BackendEngine {
    // 10. Chaos Injection ---------------------------------------------------
    pub fn add_chaos_rule(&self, rule: ChaosRule) {
        self.chaos.lock().push(rule);
    }

    pub fn clear_chaos(&self) {
        self.chaos.lock().clear();
    }

    /// Decide chaos outcome deterministically from `request_id` (stable hash),
    /// so replays reproduce the exact same fault pattern (no fake RNG).
    pub fn apply_chaos(&self, url: &str, request_id: &str) -> ChaosOutcome {
        let mut outcome = ChaosOutcome {
            injected_fault: false,
            added_latency_ms: 0,
            forced_status: None,
        };
        for rule in self.chaos.lock().iter() {
            if !url.contains(&rule.url_pattern) {
                continue;
            }
            outcome.added_latency_ms += rule.added_latency_ms;
            if rule.error_rate > 0.0 {
                let bucket = (fnv1a(request_id) % 10_000) as f64 / 10_000.0;
                if bucket < rule.error_rate {
                    outcome.injected_fault = true;
                    outcome.forced_status = Some(if rule.error_status > 0 {
                        rule.error_status
                    } else {
                        503
                    });
                }
            }
        }
        outcome
    }

    // 11. Idempotency Auditor ----------------------------------------------
    pub fn audit_idempotency(&self, events: &[IdempotencyEvent]) -> Vec<IdempotencyFinding> {
        let mut findings = Vec::new();
        let mut seen_keys: HashMap<String, usize> = HashMap::new();
        let mut unsafe_no_key: HashMap<(String, String), usize> = HashMap::new();
        for e in events {
            let method = e.method.to_uppercase();
            let is_unsafe = matches!(method.as_str(), "POST" | "PATCH");
            match &e.idempotency_key {
                Some(k) => {
                    *seen_keys.entry(k.clone()).or_default() += 1;
                }
                None if is_unsafe => {
                    *unsafe_no_key
                        .entry((method.clone(), e.url.clone()))
                        .or_default() += 1;
                }
                None => {}
            }
        }
        for ((method, url), count) in unsafe_no_key {
            if count > 1 {
                findings.push(IdempotencyFinding {
                    url,
                    method,
                    severity: "high".into(),
                    detail: format!(
                        "{count} non-idempotent requests without an Idempotency-Key (retry unsafe)"
                    ),
                });
            }
        }
        for (key, count) in seen_keys {
            if count > 1 {
                findings.push(IdempotencyFinding {
                    url: format!("key={key}"),
                    method: "*".into(),
                    severity: "info".into(),
                    detail: format!("idempotency key reused {count}x (expected for safe retries)"),
                });
            }
        }
        findings
    }
}
/// FNV-1a 64-bit hash for deterministic chaos bucketing.
fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
