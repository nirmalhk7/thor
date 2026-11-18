use super::{
    percentile_of, BTreeMap, ConnPoolEntry, ConnPoolReport, Http3PathReport, Http3PathSample,
    PerfEngine,
};

impl PerfEngine {
    // 20. Connection Pool Inspector -----------------------------------------
    pub fn conn_pool_inspect(&self, entries: &[ConnPoolEntry]) -> ConnPoolReport {
        if entries.is_empty() {
            return ConnPoolReport {
                requests: 0,
                unique_connections: 0,
                reuse_pct: 0.0,
                avg_wait_ms: 0.0,
                p95_wait_ms: 0.0,
                hottest_connection: String::new(),
                requests_on_hottest: 0,
            };
        }
        let reused = entries.iter().filter(|e| e.reused).count();
        let reuse_pct = reused as f64 / entries.len() as f64 * 100.0;
        let waits: Vec<f64> = entries.iter().map(|e| e.wait_ms).collect();
        let avg_wait_ms = waits.iter().sum::<f64>() / waits.len() as f64;
        let p95_wait_ms = percentile_of(&waits, 95.0);
        let mut by_conn: BTreeMap<String, usize> = BTreeMap::new();
        for e in entries {
            *by_conn.entry(e.connection_id.clone()).or_default() += 1;
        }
        let unique_connections = by_conn.len();
        let (hottest_connection, requests_on_hottest) = by_conn
            .into_iter()
            .max_by_key(|(_, c)| *c)
            .unwrap_or_default();
        ConnPoolReport {
            requests: entries.len(),
            unique_connections,
            reuse_pct,
            avg_wait_ms,
            p95_wait_ms,
            hottest_connection,
            requests_on_hottest,
        }
    }

    // 21. HTTP/3 Path Audit -------------------------------------------------
    pub fn http3_path_audit(&self, samples: &[Http3PathSample]) -> Http3PathReport {
        if samples.is_empty() {
            return Http3PathReport {
                samples: 0,
                h3_pct: 0.0,
                h2_pct: 0.0,
                other_pct: 0.0,
                avg_h3_ttfb_ms: 0.0,
                avg_h2_ttfb_ms: 0.0,
                avg_fallback_cost_ms: 0.0,
                fallbacks: 0,
            };
        }
        let n = samples.len() as f64;
        let classify = |alpn: Option<&str>| -> &str {
            let a = alpn.unwrap_or("").to_lowercase();
            if a.contains("h3") || a.contains("http/3") {
                "h3"
            } else if a.contains("h2") || a.contains("http/2") {
                "h2"
            } else {
                "other"
            }
        };
        let mut h3 = 0usize;
        let mut h2 = 0usize;
        let mut other = 0usize;
        let mut h3_ttfb = Vec::new();
        let mut h2_ttfb = Vec::new();
        let mut fallback_costs = Vec::new();
        for s in samples {
            match classify(s.alpn.as_deref()) {
                "h3" => {
                    h3 += 1;
                    h3_ttfb.push(s.ttfb_ms);
                }
                "h2" => {
                    h2 += 1;
                    h2_ttfb.push(s.ttfb_ms);
                }
                _ => other += 1,
            }
            if let Some(cost) = s.fallback_cost_ms {
                fallback_costs.push(cost);
            } else if s.fallback_from.is_some() {
                // Implicit: difference vs peer h3 average left for later; count fallback.
                fallback_costs.push(0.0);
            }
        }
        let avg = |xs: &[f64]| {
            if xs.is_empty() {
                0.0
            } else {
                xs.iter().sum::<f64>() / xs.len() as f64
            }
        };
        Http3PathReport {
            samples: samples.len(),
            h3_pct: h3 as f64 / n * 100.0,
            h2_pct: h2 as f64 / n * 100.0,
            other_pct: other as f64 / n * 100.0,
            avg_h3_ttfb_ms: avg(&h3_ttfb),
            avg_h2_ttfb_ms: avg(&h2_ttfb),
            avg_fallback_cost_ms: avg(&fallback_costs),
            fallbacks: samples
                .iter()
                .filter(|s| s.fallback_from.is_some() || s.fallback_cost_ms.is_some())
                .count(),
        }
    }
}
