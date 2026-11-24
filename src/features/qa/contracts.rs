use super::{
    ActualInteraction, ContractDriftChange, ContractDriftReport, ContractInteraction,
    ContractResult, QaEngine,
};

impl QaEngine {
    // 7. Contract Testing ---------------------------------------------------
    pub fn verify_contract(
        &self,
        interactions: &[ContractInteraction],
        actuals: &[ActualInteraction],
    ) -> ContractResult {
        let mut failures = Vec::new();
        let mut verified = 0;
        for expected in interactions {
            let matched = actuals.iter().find(|a| {
                a.method.eq_ignore_ascii_case(&expected.method) && a.path == expected.path
            });
            match matched {
                None => failures.push(format!(
                    "no actual interaction for {} {}",
                    expected.method, expected.path
                )),
                Some(actual) => {
                    verified += 1;
                    if actual.status != expected.expected_status {
                        failures.push(format!(
                            "{} {}: status {} != expected {}",
                            expected.method, expected.path, actual.status, expected.expected_status
                        ));
                    }
                    if let Some(schema) = &expected.response_schema {
                        let Some(body) = &actual.body else {
                            failures.push(format!(
                                "{} {}: observed response body is missing; expected JSON to validate",
                                expected.method, expected.path
                            ));
                            continue;
                        };
                        let errs = crate::features::backend::BackendEngine::new()
                            .validate_contract(body, schema);
                        for e in errs {
                            failures.push(format!("{} {}: {}", expected.method, expected.path, e));
                        }
                    }
                }
            }
        }
        ContractResult {
            satisfied: failures.is_empty(),
            failures,
            verified,
        }
    }

    /// Diff a baseline contract against live traffic / OpenAPI-shaped actuals and
    /// report breaking changes (removed paths, status shifts, schema breaks).
    pub fn contract_drift_gate(
        &self,
        baseline: &[ContractInteraction],
        live: &[ActualInteraction],
    ) -> ContractDriftReport {
        let mut changes = Vec::new();
        for expected in baseline {
            let matched = live.iter().find(|a| {
                a.method.eq_ignore_ascii_case(&expected.method) && a.path == expected.path
            });
            match matched {
                None => changes.push(ContractDriftChange {
                    path: format!("{} {}", expected.method, expected.path),
                    kind: "removed".into(),
                    detail: "baseline interaction missing from live traffic".into(),
                    breaking: true,
                }),
                Some(actual) => {
                    if actual.status != expected.expected_status {
                        changes.push(ContractDriftChange {
                            path: format!("{} {}", expected.method, expected.path),
                            kind: "status-changed".into(),
                            detail: format!("{} → {}", expected.expected_status, actual.status),
                            breaking: true,
                        });
                    }
                    if let (Some(schema), Some(body)) = (&expected.response_schema, &actual.body) {
                        let errs = crate::features::backend::BackendEngine::new()
                            .validate_contract(body, schema);
                        for e in errs {
                            changes.push(ContractDriftChange {
                                path: format!("{} {}", expected.method, expected.path),
                                kind: "schema-breaking".into(),
                                detail: e,
                                breaking: true,
                            });
                        }
                    }
                }
            }
        }
        for actual in live {
            let known = baseline
                .iter()
                .any(|b| b.method.eq_ignore_ascii_case(&actual.method) && b.path == actual.path);
            if !known {
                changes.push(ContractDriftChange {
                    path: format!("{} {}", actual.method, actual.path),
                    kind: "added".into(),
                    detail: "live path not in baseline contract".into(),
                    breaking: false,
                });
            }
        }
        ContractDriftReport {
            breaking: changes.iter().any(|c| c.breaking),
            changes,
            baseline_count: baseline.len(),
            live_count: live.len(),
        }
    }
}
