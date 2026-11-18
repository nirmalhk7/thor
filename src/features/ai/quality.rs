use super::{
    tokenize_words, AiEngine, EvalDatasetMineReport, EvalMineCase, HallucinationCheck,
    HallucinationOracleReport, LocalCloudParityReport, ParityRun, ProtocolQuirkDiffReport,
    ProtocolQuirkSample,
};
use std::collections::BTreeMap;

impl AiEngine {
    pub fn eval_dataset_miner(&self, cases: &[EvalMineCase]) -> EvalDatasetMineReport {
        let mut tag_histogram: BTreeMap<String, usize> = BTreeMap::new();
        let mut score_sum = 0.0;
        let mut score_n = 0usize;
        for c in cases {
            for t in &c.tags {
                *tag_histogram.entry(t.clone()).or_default() += 1;
            }
            if let Some(s) = c.score {
                score_sum += s;
                score_n += 1;
            }
        }
        let avg_score = if score_n == 0 {
            None
        } else {
            Some(score_sum / score_n as f64)
        };
        // Suggested 80/10/10 split by count.
        let n = cases.len();
        let mut suggested_splits = BTreeMap::new();
        suggested_splits.insert("train".into(), n * 80 / 100);
        suggested_splits.insert("val".into(), n * 10 / 100);
        suggested_splits.insert("test".into(), n - (n * 80 / 100) - (n * 10 / 100));
        EvalDatasetMineReport {
            cases: n,
            avg_score,
            tag_histogram,
            suggested_splits,
        }
    }

    pub fn hallucination_oracle(&self, checks: &[HallucinationCheck]) -> HallucinationOracleReport {
        // Evidence rule: a claim is unsupported when flagged OR when no evidence
        // snippet shares a content token with the claim (empty evidence ⇒ unsupported).
        let unsupported_claims: Vec<String> = checks
            .iter()
            .filter(|c| {
                if c.unsupported || c.evidence_snippets.is_empty() {
                    return true;
                }
                let claim_toks = tokenize_words(&c.claim);
                !c.evidence_snippets.iter().any(|ev| {
                    let ev_toks = tokenize_words(ev);
                    claim_toks
                        .iter()
                        .any(|t| ev_toks.contains(t) && t.len() > 2)
                })
            })
            .map(|c| c.claim.clone())
            .collect();
        let unsupported = unsupported_claims.len();
        let risk = if unsupported == 0 {
            "low"
        } else if unsupported * 100 / checks.len().max(1) < 20 {
            "medium"
        } else {
            "high"
        };
        HallucinationOracleReport {
            claims: checks.len(),
            unsupported,
            risk: risk.to_string(),
            unsupported_claims,
        }
    }

    pub fn protocol_quirk_diff(&self, samples: &[ProtocolQuirkSample]) -> ProtocolQuirkDiffReport {
        let mut by_field: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut providers = BTreeMap::<String, ()>::new();
        for s in samples {
            providers.insert(s.provider.clone(), ());
            by_field
                .entry(s.field.clone())
                .or_default()
                .insert(s.provider.clone(), s.value.clone());
        }
        let mut quirks = Vec::new();
        for (field, map) in by_field {
            let values: std::collections::HashSet<_> = map.values().cloned().collect();
            if values.len() > 1 {
                quirks.push(format!("{field}: {:?}", map));
            }
        }
        ProtocolQuirkDiffReport {
            quirks,
            providers: providers.len(),
        }
    }

    pub fn local_cloud_parity(&self, runs: &[ParityRun]) -> LocalCloudParityReport {
        let local = runs.iter().find(|r| r.environment == "local");
        let cloud = runs.iter().find(|r| r.environment == "cloud");
        let mut issues = Vec::new();
        let (matched, token_delta, latency_delta_ms) = match (local, cloud) {
            (Some(l), Some(c)) => {
                let matched = l.output.trim() == c.output.trim();
                if !matched {
                    issues.push("output mismatch between local and cloud".into());
                }
                let td = c.tokens as i64 - l.tokens as i64;
                if td.abs() > 5 {
                    issues.push(format!("token delta {td}"));
                }
                let ld = c.latency_ms as i64 - l.latency_ms as i64;
                (matched, td, ld)
            }
            _ => {
                issues.push("need both local and cloud runs".into());
                (false, 0, 0)
            }
        };
        LocalCloudParityReport {
            matched,
            token_delta,
            latency_delta_ms,
            issues,
        }
    }
}
