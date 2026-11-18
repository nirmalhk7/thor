use super::{OracleObservation, OracleSpec, QaEngine};

impl QaEngine {
    /// Synthesize oracles from DOM / status / headers / vitals / storage
    /// postconditions observed after a step.
    pub fn synthesize_oracles(&self, obs: &OracleObservation) -> Vec<OracleSpec> {
        let mut out = Vec::new();
        if let Some(dom) = &obs.dom_digest {
            out.push(OracleSpec {
                kind: "dom".into(),
                assertion: format!("expect(domDigest).toBe('{dom}')"),
                source: "dom_digest".into(),
            });
        }
        if let Some(status) = obs.status {
            out.push(OracleSpec {
                kind: "status".into(),
                assertion: format!("expect(response.status()).toBe({status})"),
                source: "status".into(),
            });
        }
        for (k, v) in &obs.headers {
            out.push(OracleSpec {
                kind: "header".into(),
                assertion: format!(
                    "expect(response.headers()['{}']).toBe('{}')",
                    k.to_lowercase(),
                    v.replace('\'', "\\'")
                ),
                source: format!("header:{k}"),
            });
        }
        if let Some(v) = &obs.vitals {
            out.push(OracleSpec {
                kind: "vitals".into(),
                assertion: format!(
                    "expect(vitals.lcp_ms).toBeLessThanOrEqual({}); expect(vitals.cls).toBeLessThanOrEqual({})",
                    v.lcp_ms.max(1.0),
                    (v.cls + 0.01).max(0.01)
                ),
                source: "vitals".into(),
            });
        }
        for (k, v) in &obs.storage {
            out.push(OracleSpec {
                kind: "storage".into(),
                assertion: format!(
                    "expect(await page.evaluate(() => localStorage.getItem('{}'))).toBe('{}')",
                    k.replace('\'', "\\'"),
                    v.replace('\'', "\\'")
                ),
                source: format!("storage:{k}"),
            });
        }
        out
    }
}
