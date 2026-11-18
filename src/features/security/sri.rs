use super::{SecurityEngine, SriAuditReport, SriScript};

impl SecurityEngine {
    pub fn sri_supply_chain_auditor(&self, scripts: &[SriScript]) -> SriAuditReport {
        let mut missing_integrity = Vec::new();
        let mut weak_algo = Vec::new();
        let mut missing_crossorigin = Vec::new();
        for s in scripts {
            match &s.integrity {
                None => missing_integrity.push(s.src.clone()),
                Some(integ) => {
                    let lower = integ.to_lowercase();
                    if lower.starts_with("sha256-")
                        || lower.starts_with("sha384-")
                        || lower.starts_with("sha512-")
                    {
                        // ok
                    } else {
                        weak_algo.push(s.src.clone());
                    }
                    if s.crossorigin.is_none() {
                        missing_crossorigin.push(s.src.clone());
                    }
                }
            }
        }
        SriAuditReport {
            missing_integrity,
            weak_algo,
            missing_crossorigin,
        }
    }
}
