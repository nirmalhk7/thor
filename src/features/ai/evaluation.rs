use super::{AiEngine, EvalCase, EvalResult, ReplayReport};
use crate::dx::test_generator::RecordedSession;

impl AiEngine {
    // 5. Eval Replay --------------------------------------------------------
    pub fn eval_replay(&self, cases: &[EvalCase]) -> EvalResult {
        let mut passed = 0;
        let mut failures = Vec::new();
        for c in cases {
            let ok = match c.match_mode.as_deref().unwrap_or("exact") {
                "contains" => c.actual.contains(&c.expected),
                "ci" => c.actual.trim().eq_ignore_ascii_case(c.expected.trim()),
                _ => c.actual == c.expected,
            };
            if ok {
                passed += 1;
            } else {
                failures.push(c.name.clone());
            }
        }
        let total = cases.len();
        EvalResult {
            total,
            passed,
            pass_rate: if total > 0 {
                passed as f64 / total as f64
            } else {
                0.0
            },
            failures,
        }
    }

    // 12. Deterministic Replay ----------------------------------------------
    pub fn deterministic_replay(&self, outputs: &[String], temperature: f64) -> ReplayReport {
        let mut distinct = std::collections::HashSet::new();
        for o in outputs {
            distinct.insert(o.clone());
        }
        let deterministic = distinct.len() <= 1;
        let expected_deterministic = temperature <= f64::EPSILON;
        let verdict = match (deterministic, expected_deterministic) {
            (true, true) => "OK: deterministic as expected",
            (false, true) => "VIOLATION: temperature=0 but outputs diverged",
            (true, false) => "STABLE: deterministic despite temperature>0",
            (false, false) => "OK: variation expected at temperature>0",
        };
        ReplayReport {
            runs: outputs.len(),
            distinct_outputs: distinct.len(),
            deterministic,
            expected_deterministic,
            verdict: verdict.to_string(),
        }
    }

    // Eval from recorded session -------------------------------------------
    /// Build and run an eval suite from a recorded session: every recorded
    /// network call becomes a case asserting it completed with a success status,
    /// turning an ad-hoc capture into a regression-checkable eval.
    pub fn eval_from_session(&self, session: &RecordedSession) -> EvalResult {
        let cases: Vec<EvalCase> = session
            .traces
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let ok = t.status > 0 && t.status < 400;
                EvalCase {
                    name: format!("{} {} {}", i + 1, t.method, t.url),
                    expected: "success".to_string(),
                    actual: if ok {
                        "success".to_string()
                    } else {
                        format!("status {}", t.status)
                    },
                    match_mode: Some("exact".to_string()),
                }
            })
            .collect();
        self.eval_replay(&cases)
    }
}
